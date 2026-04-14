use std::collections::HashSet;
use std::sync::Arc;

use hypr_db_core2::Db3;

use crate::query::run_query;
use crate::watch::{DependencyWatchIndex, WatchId};
use crate::{DependencyAnalysis, DependencyTarget, QueryEventSink, SubscriptionRegistration};

struct Subscription<S> {
    id: String,
    sql: String,
    params: Vec<serde_json::Value>,
    sink: S,
    lifecycle: ReactiveLifecycle,
}

enum SubscriptionSlot {
    Reactive(WatchId),
    NonReactive(String),
}

enum ReactiveLifecycle {
    Initializing,
    Active { ignore_through_seq: u64 },
}

struct Inner<S> {
    deps: DependencyWatchIndex,
    ids: std::collections::HashMap<String, SubscriptionSlot>,
    subscriptions: std::collections::HashMap<WatchId, Subscription<S>>,
}

impl<S> Default for Inner<S> {
    fn default() -> Self {
        Self {
            deps: DependencyWatchIndex::default(),
            ids: std::collections::HashMap::new(),
            subscriptions: std::collections::HashMap::new(),
        }
    }
}

#[derive(Clone)]
pub(crate) struct Registry<S> {
    inner: Arc<tokio::sync::Mutex<Inner<S>>>,
}

impl<S> Default for Registry<S> {
    fn default() -> Self {
        Self {
            inner: Arc::new(tokio::sync::Mutex::new(Inner::default())),
        }
    }
}

#[derive(Clone)]
pub(crate) struct RefreshJob {
    pub(crate) watch_id: WatchId,
    pub(crate) sql: String,
    pub(crate) params: Vec<serde_json::Value>,
}

pub(crate) struct RegisteredSubscription {
    pub(crate) registration: SubscriptionRegistration,
    pub(crate) reactive_watch_id: Option<WatchId>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum QueryEventPayload {
    Result(Vec<serde_json::Value>),
    Error(String),
}

impl QueryEventPayload {
    pub(crate) async fn load(db: &Db3, sql: &str, params: &[serde_json::Value]) -> Self {
        match run_query(db, sql, params).await {
            Ok(rows) => Self::Result(rows),
            Err(error) => Self::Error(error.to_string()),
        }
    }

    pub(crate) fn send_to<S: QueryEventSink>(&self, sink: &S) -> std::result::Result<(), String> {
        match self {
            Self::Result(rows) => sink.send_result(rows.clone()),
            Self::Error(error) => sink.send_error(error.clone()),
        }
    }
}

impl<S> Registry<S> {
    pub(crate) async fn register(
        &self,
        sql: String,
        params: Vec<serde_json::Value>,
        sink: S,
        analysis: DependencyAnalysis,
    ) -> RegisteredSubscription {
        let subscription_id = uuid::Uuid::new_v4().to_string();
        let subscription = Subscription {
            id: subscription_id.clone(),
            sql,
            params,
            sink,
            lifecycle: ReactiveLifecycle::Initializing,
        };

        let mut inner = self.inner.lock().await;
        let reactive_watch_id = match &analysis {
            DependencyAnalysis::Reactive { targets } => {
                let watch_id = inner.deps.register(targets.clone());
                inner.ids.insert(
                    subscription_id.clone(),
                    SubscriptionSlot::Reactive(watch_id),
                );
                inner.subscriptions.insert(watch_id, subscription);
                Some(watch_id)
            }
            DependencyAnalysis::NonReactive { reason } => {
                inner.ids.insert(
                    subscription_id.clone(),
                    SubscriptionSlot::NonReactive(reason.clone()),
                );
                drop(subscription);
                None
            }
        };

        RegisteredSubscription {
            registration: SubscriptionRegistration {
                id: subscription_id,
                analysis,
            },
            reactive_watch_id,
        }
    }

    pub(crate) async fn unregister(&self, subscription_id: &str) -> bool {
        let mut inner = self.inner.lock().await;
        match inner.ids.remove(subscription_id) {
            Some(SubscriptionSlot::Reactive(watch_id)) => {
                inner.subscriptions.remove(&watch_id);
                inner.deps.unregister(watch_id);
                true
            }
            Some(SubscriptionSlot::NonReactive(_)) => true,
            None => false,
        }
    }

    pub(crate) async fn dependency_analysis(
        &self,
        subscription_id: &str,
    ) -> Option<DependencyAnalysis> {
        let inner = self.inner.lock().await;
        match inner.ids.get(subscription_id) {
            Some(SubscriptionSlot::Reactive(watch_id)) => {
                inner.subscriptions.get(watch_id).map(|_| {
                    let targets = inner.deps.targets_for(*watch_id).unwrap_or_default();
                    DependencyAnalysis::Reactive { targets }
                })
            }
            Some(SubscriptionSlot::NonReactive(reason)) => Some(DependencyAnalysis::NonReactive {
                reason: reason.clone(),
            }),
            None => None,
        }
    }

    pub(crate) async fn collect_jobs(
        &self,
        changed_targets: &HashSet<DependencyTarget>,
        trigger_seq: u64,
    ) -> Vec<RefreshJob> {
        let inner = self.inner.lock().await;
        inner
            .deps
            .affected(&changed_targets.iter().cloned().collect::<Vec<_>>())
            .into_iter()
            .filter_map(|id| try_build_job(id, inner.subscriptions.get(&id)?, trigger_seq))
            .collect()
    }

    pub(crate) async fn collect_all_jobs(&self, trigger_seq: u64) -> Vec<RefreshJob> {
        let inner = self.inner.lock().await;
        inner
            .subscriptions
            .iter()
            .filter_map(|(&id, sub)| try_build_job(id, sub, trigger_seq))
            .collect()
    }

    pub(crate) async fn activate(&self, watch_id: WatchId, ignore_through_seq: u64) -> bool {
        let mut inner = self.inner.lock().await;
        let Some(subscription) = inner.subscriptions.get_mut(&watch_id) else {
            return false;
        };

        match subscription.lifecycle {
            ReactiveLifecycle::Initializing => {
                subscription.lifecycle = ReactiveLifecycle::Active { ignore_through_seq };
                true
            }
            ReactiveLifecycle::Active { .. } => false,
        }
    }
}

impl<S: QueryEventSink> Registry<S> {
    pub(crate) async fn refresh(
        &self,
        db: &Db3,
        job: RefreshJob,
        suppress_if_equal: Option<&QueryEventPayload>,
    ) {
        let payload = QueryEventPayload::load(db, &job.sql, &job.params).await;

        let mut inner = self.inner.lock().await;
        let (subscription_id, send_result) = {
            let Some(subscription) = inner.subscriptions.get(&job.watch_id) else {
                return;
            };

            if !matches!(subscription.lifecycle, ReactiveLifecycle::Active { .. }) {
                return;
            }

            if suppress_if_equal == Some(&payload) {
                return;
            }

            (subscription.id.clone(), payload.send_to(&subscription.sink))
        };

        if send_result.is_err() {
            remove_subscription(&mut inner, job.watch_id, subscription_id);
        }
    }
}

fn try_build_job<S>(
    watch_id: WatchId,
    sub: &Subscription<S>,
    trigger_seq: u64,
) -> Option<RefreshJob> {
    match sub.lifecycle {
        ReactiveLifecycle::Active { ignore_through_seq } if trigger_seq > ignore_through_seq => {
            Some(RefreshJob {
                watch_id,
                sql: sub.sql.clone(),
                params: sub.params.clone(),
            })
        }
        _ => None,
    }
}

fn remove_subscription<S>(inner: &mut Inner<S>, watch_id: WatchId, subscription_id: String) {
    inner.subscriptions.remove(&watch_id);
    inner.ids.remove(&subscription_id);
    inner.deps.unregister(watch_id);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone)]
    struct TestSink;

    impl QueryEventSink for TestSink {
        fn send_result(&self, _rows: Vec<serde_json::Value>) -> std::result::Result<(), String> {
            Ok(())
        }

        fn send_error(&self, _error: String) -> std::result::Result<(), String> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn delayed_old_broadcasts_are_ignored_after_activation() {
        let registry = Registry::<TestSink>::default();
        let registered = registry
            .register(
                "SELECT id FROM daily_notes".to_string(),
                vec![],
                TestSink,
                DependencyAnalysis::Reactive {
                    targets: HashSet::from([DependencyTarget::Table("daily_notes".to_string())]),
                },
            )
            .await;
        let watch_id = registered.reactive_watch_id.unwrap();

        assert!(registry.activate(watch_id, 11).await);

        let jobs = registry
            .collect_jobs(
                &HashSet::from([DependencyTarget::Table("daily_notes".to_string())]),
                11,
            )
            .await;

        assert!(jobs.is_empty());
    }

    #[tokio::test]
    async fn newer_broadcasts_enqueue_after_activation() {
        let registry = Registry::<TestSink>::default();
        let registered = registry
            .register(
                "SELECT id FROM daily_notes".to_string(),
                vec![],
                TestSink,
                DependencyAnalysis::Reactive {
                    targets: HashSet::from([DependencyTarget::Table("daily_notes".to_string())]),
                },
            )
            .await;
        let watch_id = registered.reactive_watch_id.unwrap();

        assert!(registry.activate(watch_id, 11).await);

        let jobs = registry
            .collect_jobs(
                &HashSet::from([DependencyTarget::Table("daily_notes".to_string())]),
                12,
            )
            .await;

        assert_eq!(jobs.len(), 1);
    }
}

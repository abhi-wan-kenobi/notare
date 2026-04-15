use std::collections::HashSet;
use std::sync::Arc;

use hypr_db_core::Db;

use crate::query::run_query;
use crate::watch::{DependencyWatchIndex, WatchId};
use crate::{DependencyAnalysis, DependencyTarget, QueryEventSink, SubscriptionRegistration};

struct Subscription<S> {
    analysis: DependencyAnalysis,
    state: SubscriptionState<S>,
}

enum ReactiveLifecycle {
    Initializing,
    Active { ignore_through_seq: u64 },
}

enum SubscriptionState<S> {
    Reactive(ReactiveSubscription<S>),
    NonReactive,
}

struct ReactiveSubscription<S> {
    watch_id: WatchId,
    sql: String,
    params: Vec<serde_json::Value>,
    sink: S,
    lifecycle: ReactiveLifecycle,
}

struct Inner<S> {
    deps: DependencyWatchIndex,
    subscriptions: std::collections::HashMap<String, Subscription<S>>,
    watch_ids: std::collections::HashMap<WatchId, String>,
}

impl<S> Default for Inner<S> {
    fn default() -> Self {
        Self {
            deps: DependencyWatchIndex::default(),
            subscriptions: std::collections::HashMap::new(),
            watch_ids: std::collections::HashMap::new(),
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
    pub(crate) async fn load(db: &Db, sql: &str, params: &[serde_json::Value]) -> Self {
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
        let mut inner = self.inner.lock().await;
        let state = match &analysis {
            DependencyAnalysis::Reactive { targets } => {
                let watch_id = inner.deps.register(targets.clone());
                inner.watch_ids.insert(watch_id, subscription_id.clone());
                SubscriptionState::Reactive(ReactiveSubscription {
                    watch_id,
                    sql,
                    params,
                    sink,
                    lifecycle: ReactiveLifecycle::Initializing,
                })
            }
            DependencyAnalysis::NonReactive { .. } => SubscriptionState::NonReactive,
        };
        let reactive_watch_id = state.watch_id();

        inner.subscriptions.insert(
            subscription_id.clone(),
            Subscription {
                analysis: analysis.clone(),
                state,
            },
        );

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
        remove_subscription(&mut inner, subscription_id)
    }

    pub(crate) async fn dependency_analysis(
        &self,
        subscription_id: &str,
    ) -> Option<DependencyAnalysis> {
        self.inner
            .lock()
            .await
            .subscriptions
            .get(subscription_id)
            .map(|subscription| subscription.analysis.clone())
    }

    pub(crate) async fn collect_jobs(
        &self,
        changed_targets: &HashSet<DependencyTarget>,
        trigger_seq: u64,
    ) -> Vec<RefreshJob> {
        let inner = self.inner.lock().await;
        inner
            .deps
            .affected(changed_targets)
            .into_iter()
            .filter_map(|watch_id| {
                let subscription_id = inner.watch_ids.get(&watch_id)?;
                let subscription = inner.subscriptions.get(subscription_id)?;
                try_build_job(subscription, trigger_seq)
            })
            .collect()
    }

    pub(crate) async fn collect_all_jobs(&self, trigger_seq: u64) -> Vec<RefreshJob> {
        let inner = self.inner.lock().await;
        inner
            .subscriptions
            .values()
            .filter_map(|subscription| try_build_job(subscription, trigger_seq))
            .collect()
    }

    pub(crate) async fn activate(&self, watch_id: WatchId, ignore_through_seq: u64) -> bool {
        let mut inner = self.inner.lock().await;
        let Some(subscription_id) = inner.watch_ids.get(&watch_id).cloned() else {
            return false;
        };
        let Some(subscription) = inner.subscriptions.get_mut(&subscription_id) else {
            return false;
        };

        match &mut subscription.state {
            SubscriptionState::Reactive(reactive) => match reactive.lifecycle {
                ReactiveLifecycle::Initializing => {
                    reactive.lifecycle = ReactiveLifecycle::Active { ignore_through_seq };
                    true
                }
                ReactiveLifecycle::Active { .. } => false,
            },
            SubscriptionState::NonReactive => false,
        }
    }
}

impl<S: QueryEventSink> Registry<S> {
    pub(crate) async fn refresh(
        &self,
        db: &Db,
        job: RefreshJob,
        suppress_if_equal: Option<&QueryEventPayload>,
    ) {
        let payload = QueryEventPayload::load(db, &job.sql, &job.params).await;

        let mut inner = self.inner.lock().await;
        let Some(subscription_id) = inner.watch_ids.get(&job.watch_id).cloned() else {
            return;
        };
        let send_result = {
            let Some(subscription) = inner.subscriptions.get(&subscription_id) else {
                return;
            };
            let SubscriptionState::Reactive(reactive) = &subscription.state else {
                return;
            };

            if !matches!(reactive.lifecycle, ReactiveLifecycle::Active { .. }) {
                return;
            }

            if suppress_if_equal == Some(&payload) {
                return;
            }

            payload.send_to(&reactive.sink)
        };

        if send_result.is_err() {
            remove_subscription(&mut inner, &subscription_id);
        }
    }
}

impl<S> SubscriptionState<S> {
    fn watch_id(&self) -> Option<WatchId> {
        match self {
            Self::Reactive(reactive) => Some(reactive.watch_id),
            Self::NonReactive => None,
        }
    }
}

fn try_build_job<S>(subscription: &Subscription<S>, trigger_seq: u64) -> Option<RefreshJob> {
    let SubscriptionState::Reactive(reactive) = &subscription.state else {
        return None;
    };

    match reactive.lifecycle {
        ReactiveLifecycle::Active { ignore_through_seq } if trigger_seq > ignore_through_seq => {
            Some(RefreshJob {
                watch_id: reactive.watch_id,
                sql: reactive.sql.clone(),
                params: reactive.params.clone(),
            })
        }
        _ => None,
    }
}

fn remove_subscription<S>(inner: &mut Inner<S>, subscription_id: &str) -> bool {
    let Some(subscription) = inner.subscriptions.remove(subscription_id) else {
        return false;
    };

    if let SubscriptionState::Reactive(reactive) = subscription.state {
        inner.watch_ids.remove(&reactive.watch_id);
        inner.deps.unregister(reactive.watch_id);
    }

    true
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

    #[tokio::test]
    async fn reactive_registrations_report_stored_targets() {
        let registry = Registry::<TestSink>::default();
        let targets = HashSet::from([DependencyTarget::Table("daily_notes".to_string())]);
        let registered = registry
            .register(
                "SELECT id FROM daily_notes".to_string(),
                vec![],
                TestSink,
                DependencyAnalysis::Reactive {
                    targets: targets.clone(),
                },
            )
            .await;

        assert_eq!(
            registry
                .dependency_analysis(&registered.registration.id)
                .await,
            Some(DependencyAnalysis::Reactive { targets })
        );
    }

    #[tokio::test]
    async fn non_reactive_registrations_report_reason_and_unregister_cleanly() {
        let registry = Registry::<TestSink>::default();
        let registered = registry
            .register(
                "SELECT 1".to_string(),
                vec![],
                TestSink,
                DependencyAnalysis::NonReactive {
                    reason: "query has no reactive dependencies".to_string(),
                },
            )
            .await;

        assert!(registered.reactive_watch_id.is_none());
        assert_eq!(
            registry
                .dependency_analysis(&registered.registration.id)
                .await,
            Some(DependencyAnalysis::NonReactive {
                reason: "query has no reactive dependencies".to_string(),
            })
        );

        assert!(registry.unregister(&registered.registration.id).await);
        assert!(
            registry
                .dependency_analysis(&registered.registration.id)
                .await
                .is_none()
        );
    }

    #[tokio::test]
    async fn collect_all_jobs_only_returns_active_subscriptions() {
        let registry = Registry::<TestSink>::default();
        let initializing = registry
            .register(
                "SELECT id FROM daily_notes".to_string(),
                vec![],
                TestSink,
                DependencyAnalysis::Reactive {
                    targets: HashSet::from([DependencyTarget::Table("daily_notes".to_string())]),
                },
            )
            .await;
        let active = registry
            .register(
                "SELECT id FROM daily_summaries".to_string(),
                vec![],
                TestSink,
                DependencyAnalysis::Reactive {
                    targets: HashSet::from([DependencyTarget::Table(
                        "daily_summaries".to_string(),
                    )]),
                },
            )
            .await;
        let active_watch_id = active.reactive_watch_id.unwrap();

        assert!(registry.activate(active_watch_id, 3).await);

        let jobs = registry.collect_all_jobs(4).await;

        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].watch_id, active_watch_id);
        assert_eq!(jobs[0].sql, "SELECT id FROM daily_summaries");
        assert!(jobs[0].params.is_empty());
        assert_ne!(initializing.reactive_watch_id, Some(active_watch_id));
    }
}

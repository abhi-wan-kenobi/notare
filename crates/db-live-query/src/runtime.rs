use std::collections::HashSet;
use std::sync::Arc;

use tokio::sync::broadcast::error::{RecvError, TryRecvError};
use tokio::sync::watch as tokio_watch;

use hypr_db_core::Db;

use crate::error::{Error, Result};
use crate::query::{run_query, run_query_proxy};
use crate::schema::CatalogStore;
use crate::subscriptions::{QueryEventPayload, RefreshJob, Registry};
use crate::types::{
    DependencyAnalysis, ProxyQueryMethod, ProxyQueryResult, QueryEventSink,
    SubscriptionRegistration,
};
use crate::watch::WatchId;
use hypr_db_change::{ChangeNotifier, TableChange};

pub struct DbRuntime<S> {
    db: Arc<Db>,
    change_notifier: ChangeNotifier,
    catalog: CatalogStore,
    subscriptions: Registry<S>,
    shutdown_tx: tokio_watch::Sender<bool>,
    dispatcher: std::sync::Mutex<Option<tokio::task::JoinHandle<()>>>,
}

impl<S: QueryEventSink> DbRuntime<S> {
    pub fn new(db: Arc<Db>) -> Self {
        let change_notifier = db.change_notifier().clone();
        Self::new_with_notifier(db, change_notifier)
    }

    fn new_with_notifier(db: Arc<Db>, change_notifier: ChangeNotifier) -> Self {
        let catalog = CatalogStore::default();
        let subscriptions = Registry::default();
        let (shutdown_tx, mut shutdown_rx) = tokio_watch::channel(false);
        let mut change_rx = change_notifier.subscribe();
        let dispatcher_catalog = catalog.clone();
        let dispatcher_subscriptions = subscriptions.clone();
        let dispatcher_db = Arc::clone(&db);
        let dispatcher_notifier = change_notifier.clone();

        let dispatcher = tokio::spawn(async move {
            loop {
                tokio::select! {
                    changed = shutdown_rx.changed() => {
                        if changed.is_err() || *shutdown_rx.borrow() {
                            break;
                        }
                    }
                    jobs = next_refresh_jobs(
                        &mut change_rx,
                        dispatcher_db.as_ref(),
                        &dispatcher_notifier,
                        &dispatcher_catalog,
                        &dispatcher_subscriptions,
                    ) => {
                        let Some(jobs) = jobs else {
                            break;
                        };
                        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                        if jobs.is_empty() {
                            continue;
                        }

                        for job in jobs {
                            dispatcher_subscriptions
                                .refresh(&dispatcher_db, job, None)
                                .await;
                        }
                    }
                }
            }
        });

        Self {
            db,
            change_notifier,
            catalog,
            subscriptions,
            shutdown_tx,
            dispatcher: std::sync::Mutex::new(Some(dispatcher)),
        }
    }

    pub async fn execute(
        &self,
        sql: String,
        params: Vec<serde_json::Value>,
    ) -> Result<Vec<serde_json::Value>> {
        run_query(&self.db, &sql, &params).await.map_err(Into::into)
    }

    pub async fn execute_proxy(
        &self,
        sql: String,
        params: Vec<serde_json::Value>,
        method: String,
    ) -> Result<ProxyQueryResult> {
        let method: ProxyQueryMethod = method.parse()?;
        run_query_proxy(&self.db, &sql, &params, method)
            .await
            .map_err(Into::into)
    }

    pub async fn subscribe(
        &self,
        sql: String,
        params: Vec<serde_json::Value>,
        sink: S,
    ) -> Result<SubscriptionRegistration> {
        let baseline_seq = self.change_notifier.current_seq();
        let analysis = self.analyze_subscription(&sql).await;
        let registered = self
            .subscriptions
            .register(sql.clone(), params.clone(), sink.clone(), analysis)
            .await;
        #[cfg(test)]
        test_support::before_initial_payload_load().await;
        let initial_payload = self
            .deliver_initial_payload(&registered.registration.id, &sql, &params, &sink)
            .await?;
        self.activate_reactive_subscription(
            baseline_seq,
            registered.reactive_watch_id,
            &registered.registration.analysis,
            &initial_payload,
            &sql,
            &params,
        )
        .await;

        Ok(registered.registration)
    }

    pub async fn unsubscribe(&self, subscription_id: &str) -> Result<()> {
        let removed = self.subscriptions.unregister(subscription_id).await;
        if removed {
            Ok(())
        } else {
            Err(Error::SubscriptionNotFound(subscription_id.to_string()))
        }
    }

    pub async fn dependency_analysis(&self, subscription_id: &str) -> Option<DependencyAnalysis> {
        self.subscriptions
            .dependency_analysis(subscription_id)
            .await
    }

    pub fn db(&self) -> &Db {
        self.db.as_ref()
    }

    async fn analyze_subscription(&self, sql: &str) -> DependencyAnalysis {
        match self.catalog.analyze_query(self.db.pool(), sql).await {
            Ok(targets) => DependencyAnalysis::Reactive { targets },
            Err(error) => DependencyAnalysis::NonReactive {
                reason: error.to_string(),
            },
        }
    }

    async fn deliver_initial_payload(
        &self,
        subscription_id: &str,
        sql: &str,
        params: &[serde_json::Value],
        sink: &S,
    ) -> Result<QueryEventPayload> {
        let initial_payload = QueryEventPayload::load(&self.db, sql, params).await;

        if let Err(error) = initial_payload.send_to(sink) {
            self.subscriptions.unregister(subscription_id).await;
            return Err(Error::Sink(error));
        }

        Ok(initial_payload)
    }

    async fn activate_reactive_subscription(
        &self,
        baseline_seq: u64,
        watch_id: Option<WatchId>,
        analysis: &DependencyAnalysis,
        initial_payload: &QueryEventPayload,
        sql: &str,
        params: &[serde_json::Value],
    ) {
        let Some(watch_id) = watch_id else {
            return;
        };

        let latest_dependency_seq = match analysis {
            DependencyAnalysis::Reactive { targets } => self
                .catalog
                .latest_dependency_seq(&self.change_notifier, targets, self.db.pool())
                .await
                .ok()
                .flatten()
                .unwrap_or(baseline_seq),
            DependencyAnalysis::NonReactive { .. } => baseline_seq,
        };

        self.subscriptions
            .activate(watch_id, latest_dependency_seq)
            .await;

        if latest_dependency_seq > baseline_seq {
            self.subscriptions
                .refresh(
                    &self.db,
                    RefreshJob {
                        watch_id,
                        sql: sql.to_string(),
                        params: params.to_vec(),
                    },
                    Some(initial_payload),
                )
                .await;
        }
    }
}

enum ChangeBatch {
    ChangedTables {
        changed_tables: HashSet<String>,
        trigger_seq: u64,
    },
    RerunAll {
        trigger_seq: u64,
    },
}

async fn next_refresh_jobs<S>(
    change_rx: &mut tokio::sync::broadcast::Receiver<TableChange>,
    db: &Db,
    change_notifier: &ChangeNotifier,
    catalog: &CatalogStore,
    subscriptions: &Registry<S>,
) -> Option<Vec<RefreshJob>> {
    let batch = receive_change_batch(change_rx, change_notifier).await?;
    Some(collect_refresh_jobs(db, catalog, subscriptions, batch).await)
}

async fn receive_change_batch(
    change_rx: &mut tokio::sync::broadcast::Receiver<TableChange>,
    change_notifier: &ChangeNotifier,
) -> Option<ChangeBatch> {
    match change_rx.recv().await {
        Ok(first_change) => Some(drain_buffered_changes(
            change_rx,
            change_notifier,
            first_change,
        )),
        Err(RecvError::Closed) => None,
        Err(RecvError::Lagged(_)) => Some(rerun_all_batch(change_rx, change_notifier)),
    }
}

fn drain_buffered_changes(
    change_rx: &mut tokio::sync::broadcast::Receiver<TableChange>,
    change_notifier: &ChangeNotifier,
    first_change: TableChange,
) -> ChangeBatch {
    let mut changed_tables = HashSet::from([first_change.table]);
    let mut trigger_seq = first_change.seq;

    loop {
        match change_rx.try_recv() {
            Ok(next_change) => {
                trigger_seq = trigger_seq.max(next_change.seq);
                changed_tables.insert(next_change.table);
            }
            Err(TryRecvError::Empty) | Err(TryRecvError::Closed) => {
                return ChangeBatch::ChangedTables {
                    changed_tables,
                    trigger_seq,
                };
            }
            Err(TryRecvError::Lagged(_)) => return rerun_all_batch(change_rx, change_notifier),
        }
    }
}

fn rerun_all_batch(
    change_rx: &mut tokio::sync::broadcast::Receiver<TableChange>,
    change_notifier: &ChangeNotifier,
) -> ChangeBatch {
    clear_lagged_changes(change_rx);
    ChangeBatch::RerunAll {
        trigger_seq: change_notifier.current_seq(),
    }
}

fn clear_lagged_changes(change_rx: &mut tokio::sync::broadcast::Receiver<TableChange>) {
    loop {
        match change_rx.try_recv() {
            Ok(_) | Err(TryRecvError::Lagged(_)) => {}
            Err(TryRecvError::Empty) | Err(TryRecvError::Closed) => break,
        }
    }
}

async fn collect_refresh_jobs<S>(
    db: &Db,
    catalog: &CatalogStore,
    subscriptions: &Registry<S>,
    batch: ChangeBatch,
) -> Vec<RefreshJob> {
    match batch {
        ChangeBatch::ChangedTables {
            changed_tables,
            trigger_seq,
        } => match catalog
            .canonicalize_raw_tables(db.pool(), &changed_tables)
            .await
        {
            Ok(changed_targets) => {
                subscriptions
                    .collect_jobs(&changed_targets, trigger_seq)
                    .await
            }
            Err(_) => subscriptions.collect_all_jobs(trigger_seq).await,
        },
        ChangeBatch::RerunAll { trigger_seq } => subscriptions.collect_all_jobs(trigger_seq).await,
    }
}

impl<S> Drop for DbRuntime<S> {
    fn drop(&mut self) {
        let _ = self.shutdown_tx.send(true);
        if let Some(dispatcher) = self.dispatcher.lock().unwrap().take() {
            dispatcher.abort();
        }
    }
}

#[cfg(test)]
mod test_support {
    use std::sync::Arc;
    use std::sync::OnceLock;
    use std::sync::atomic::{AtomicBool, Ordering};

    use tokio::sync::{Mutex, Notify};

    struct InitialPayloadHook {
        reached: AtomicBool,
        reached_notify: Notify,
        released: AtomicBool,
        release_notify: Notify,
    }

    impl InitialPayloadHook {
        fn new() -> Self {
            Self {
                reached: AtomicBool::new(false),
                reached_notify: Notify::new(),
                released: AtomicBool::new(false),
                release_notify: Notify::new(),
            }
        }
    }

    pub(crate) struct InitialPayloadHookHandle {
        hook: Arc<InitialPayloadHook>,
    }

    fn hook_slot() -> &'static Mutex<Option<Arc<InitialPayloadHook>>> {
        static SLOT: OnceLock<Mutex<Option<Arc<InitialPayloadHook>>>> = OnceLock::new();
        SLOT.get_or_init(|| Mutex::new(None))
    }

    pub(crate) async fn install_initial_payload_hook() -> InitialPayloadHookHandle {
        let hook = Arc::new(InitialPayloadHook::new());
        *hook_slot().lock().await = Some(Arc::clone(&hook));
        InitialPayloadHookHandle { hook }
    }

    pub(crate) async fn before_initial_payload_load() {
        let hook = hook_slot().lock().await.clone();
        let Some(hook) = hook else {
            return;
        };

        hook.reached.store(true, Ordering::SeqCst);
        hook.reached_notify.notify_waiters();
        while !hook.released.load(Ordering::SeqCst) {
            hook.release_notify.notified().await;
        }
    }

    impl InitialPayloadHookHandle {
        pub(crate) async fn wait_until_reached(&self) {
            tokio::time::timeout(std::time::Duration::from_secs(1), async {
                while !self.hook.reached.load(Ordering::SeqCst) {
                    self.hook.reached_notify.notified().await;
                }
            })
            .await
            .expect("initial payload hook should be reached");
        }

        pub(crate) async fn release(self) {
            self.hook.released.store(true, Ordering::SeqCst);
            self.hook.release_notify.notify_waiters();
            *hook_slot().lock().await = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use hypr_db_core::{DbOpenOptions, DbStorage};
    use serde_json::json;

    use super::*;
    use crate::types::QueryEventSink;

    #[derive(Clone, Debug, PartialEq)]
    enum TestEvent {
        Result(Vec<serde_json::Value>),
        Error(String),
    }

    #[derive(Clone)]
    struct TestSink {
        events: Arc<Mutex<Vec<TestEvent>>>,
    }

    impl QueryEventSink for TestSink {
        fn send_result(&self, rows: Vec<serde_json::Value>) -> std::result::Result<(), String> {
            self.events.lock().unwrap().push(TestEvent::Result(rows));
            Ok(())
        }

        fn send_error(&self, error: String) -> std::result::Result<(), String> {
            self.events.lock().unwrap().push(TestEvent::Error(error));
            Ok(())
        }
    }

    impl TestSink {
        fn capture() -> (Self, Arc<Mutex<Vec<TestEvent>>>) {
            let events = Arc::new(Mutex::new(Vec::new()));
            (
                Self {
                    events: Arc::clone(&events),
                },
                events,
            )
        }
    }

    const LIVE_QUERY_TEST_MIGRATION_STEPS: &[hypr_db_migrate::MigrationStep] =
        &[hypr_db_migrate::MigrationStep {
            id: "20260415000000_live_query_test_schema",
            scope: hypr_db_migrate::MigrationScope::Plain,
            sql: include_str!("../tests/common/live_query_test_schema.sql"),
        }];

    fn live_query_test_schema() -> hypr_db_migrate::DbSchema {
        hypr_db_migrate::DbSchema {
            steps: LIVE_QUERY_TEST_MIGRATION_STEPS,
            validate_cloudsync_table: |_| false,
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn stale_init_time_broadcast_processed_after_activation_is_ignored() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("app.db");
        let db = hypr_db_core::Db::open(DbOpenOptions {
            storage: DbStorage::Local(&db_path),
            cloudsync_enabled: false,
            journal_mode_wal: true,
            foreign_keys: true,
            max_connections: Some(4),
        })
        .await
        .unwrap();
        hypr_db_migrate::migrate(&db, live_query_test_schema())
            .await
            .unwrap();

        let pool = db.pool().clone();
        let runtime = DbRuntime::new(Arc::new(db));

        let hook = test_support::install_initial_payload_hook().await;
        let (sink, events) = TestSink::capture();

        let subscribe = tokio::spawn(async move {
            runtime
                .subscribe(
                    "SELECT id FROM daily_notes WHERE id = ?".to_string(),
                    vec![json!("note-stale-after-activation")],
                    sink,
                )
                .await
        });

        hook.wait_until_reached().await;

        sqlx::query("INSERT INTO daily_notes (id, date, body, user_id) VALUES (?, ?, ?, ?)")
            .bind("note-stale-after-activation")
            .bind("2026-04-25")
            .bind("{}")
            .bind("user-stale")
            .execute(&pool)
            .await
            .unwrap();

        for idx in 0..320 {
            sqlx::query("INSERT INTO daily_notes (id, date, body, user_id) VALUES (?, ?, ?, ?)")
                .bind(format!("note-lag-{idx}"))
                .bind("2026-04-25")
                .bind("{}")
                .bind(format!("user-lag-{idx}"))
                .execute(&pool)
                .await
                .unwrap();
        }

        hook.release().await;
        subscribe.await.unwrap().unwrap();

        let initial = tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                if let Some(event) = events.lock().unwrap().first().cloned() {
                    return event;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        assert_eq!(
            initial,
            TestEvent::Result(vec![json!({ "id": "note-stale-after-activation" })])
        );

        tokio::time::sleep(Duration::from_millis(100)).await;
        assert_eq!(events.lock().unwrap().len(), 1);
    }
}

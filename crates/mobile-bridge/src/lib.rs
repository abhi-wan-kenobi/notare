#![forbid(unsafe_code)]

mod db;
mod error;
mod listener;

use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use error::{BridgeError, execute_error, parse_params_json, reactive_error, serialization_error};
use listener::{ListenerSink, QueryEventListener};

uniffi::setup_scaffolding!();

struct BridgeState {
    executor: hypr_db_execute::DbExecutor,
    live_query_runtime: Arc<hypr_db_reactive::LiveQueryRuntime<ListenerSink>>,
    runtime: Arc<tokio::runtime::Runtime>,
    subscription_ids: HashSet<String>,
}

#[derive(uniffi::Object)]
pub struct MobileDbBridge {
    state: Mutex<Option<BridgeState>>,
}

#[uniffi::export]
impl MobileDbBridge {
    #[uniffi::constructor]
    pub fn open(db_path: String) -> Result<Self, BridgeError> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .map_err(|error| BridgeError::OpenFailed {
                reason: error.to_string(),
            })?;
        let runtime = Arc::new(runtime);
        let path = std::path::PathBuf::from(db_path);
        let db = runtime
            .handle()
            .block_on(db::open_app_db(&path))
            .map_err(|error| BridgeError::OpenFailed {
                reason: error.to_string(),
            })?;
        let db = std::sync::Arc::new(db);
        let executor = hypr_db_execute::DbExecutor::new(std::sync::Arc::clone(&db));
        let live_query_runtime = {
            let _guard = runtime.enter();
            Arc::new(hypr_db_reactive::LiveQueryRuntime::new(db))
        };

        Ok(Self {
            state: Mutex::new(Some(BridgeState {
                executor,
                live_query_runtime,
                runtime,
                subscription_ids: HashSet::new(),
            })),
        })
    }

    pub fn execute(&self, sql: String, params_json: String) -> Result<String, BridgeError> {
        let params = parse_params_json(&params_json)?;
        let (runtime, executor) =
            self.with_state(|state| Ok((Arc::clone(&state.runtime), state.executor.clone())))?;
        let rows = runtime
            .handle()
            .block_on(executor.execute(sql, params))
            .map_err(execute_error)?;
        serde_json::to_string(&rows).map_err(serialization_error)
    }

    pub fn execute_proxy(
        &self,
        sql: String,
        params_json: String,
        method: String,
    ) -> Result<String, BridgeError> {
        let params = parse_params_json(&params_json)?;
        let method = method
            .parse::<hypr_db_execute::ProxyQueryMethod>()
            .map_err(execute_error)?;
        let (runtime, executor) =
            self.with_state(|state| Ok((Arc::clone(&state.runtime), state.executor.clone())))?;
        let rows = runtime
            .handle()
            .block_on(executor.execute_proxy(sql, params, method))
            .map_err(execute_error)?;
        serde_json::to_string(&rows).map_err(serialization_error)
    }

    pub fn subscribe(
        &self,
        sql: String,
        params_json: String,
        listener: Arc<dyn QueryEventListener>,
    ) -> Result<String, BridgeError> {
        let params = parse_params_json(&params_json)?;
        let sql_for_log = sql.clone();
        let (runtime, live_query_runtime) = self.with_state(|state| {
            Ok((
                Arc::clone(&state.runtime),
                Arc::clone(&state.live_query_runtime),
            ))
        })?;
        let registration = runtime
            .handle()
            .block_on(live_query_runtime.subscribe(sql, params, ListenerSink::new(listener)))
            .map_err(reactive_error)?;

        if let hypr_db_reactive::DependencyAnalysis::NonReactive { reason } = &registration.analysis
        {
            eprintln!(
                "[mobile-bridge] live query subscription is non-reactive for SQL {:?}: {}",
                sql_for_log, reason
            );
        }

        let subscription_id = registration.id.clone();
        if self
            .with_state(|state| {
                state.subscription_ids.insert(subscription_id.clone());
                Ok(())
            })
            .is_err()
        {
            let _ = runtime
                .handle()
                .block_on(live_query_runtime.unsubscribe(&registration.id));
            return Err(BridgeError::Closed);
        }

        Ok(registration.id)
    }

    pub fn unsubscribe(&self, subscription_id: String) -> Result<(), BridgeError> {
        let (runtime, live_query_runtime) = self.with_state(|state| {
            Ok((
                Arc::clone(&state.runtime),
                Arc::clone(&state.live_query_runtime),
            ))
        })?;
        runtime
            .handle()
            .block_on(live_query_runtime.unsubscribe(&subscription_id))
            .map_err(reactive_error)?;
        self.with_state(|state| {
            state.subscription_ids.remove(&subscription_id);
            Ok(())
        })
    }

    pub fn close(&self) -> Result<(), BridgeError> {
        let mut guard = self.state.lock().unwrap();
        let Some(mut state) = guard.take() else {
            return Ok(());
        };
        drop(guard);

        let subscription_ids: Vec<String> = state.subscription_ids.drain().collect();
        let pool = state.live_query_runtime.db().pool().clone();
        state.runtime.handle().block_on(async {
            for subscription_id in subscription_ids {
                let _ = state.live_query_runtime.unsubscribe(&subscription_id).await;
            }
        });
        drop(state.live_query_runtime);
        drop(state.executor);
        state.runtime.handle().block_on(pool.close());

        Ok(())
    }
}

impl MobileDbBridge {
    fn with_state<T>(
        &self,
        f: impl FnOnce(&mut BridgeState) -> Result<T, BridgeError>,
    ) -> Result<T, BridgeError> {
        let mut guard = self.state.lock().unwrap();
        let state = guard.as_mut().ok_or(BridgeError::Closed)?;
        f(state)
    }
}

impl Drop for MobileDbBridge {
    fn drop(&mut self) {
        let _ = self.close();
    }
}

#[cfg(any(
    all(test, target_os = "macos", target_arch = "aarch64"),
    all(test, target_os = "macos", target_arch = "x86_64"),
    all(test, target_os = "linux", target_env = "gnu", target_arch = "aarch64"),
    all(test, target_os = "linux", target_env = "gnu", target_arch = "x86_64"),
    all(
        test,
        target_os = "linux",
        target_env = "musl",
        target_arch = "aarch64"
    ),
    all(test, target_os = "linux", target_env = "musl", target_arch = "x86_64"),
    all(test, target_os = "windows", target_arch = "x86_64"),
))]
mod tests {
    use super::*;
    use std::process::Command;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::Duration;

    #[derive(Clone, Debug, PartialEq)]
    enum TestEvent {
        Result(Vec<serde_json::Value>),
        Error(String),
    }

    #[derive(Clone)]
    struct TestListener {
        events: Arc<Mutex<Vec<TestEvent>>>,
    }

    impl QueryEventListener for TestListener {
        fn on_result(&self, rows_json: String) {
            let rows: Vec<serde_json::Value> =
                serde_json::from_str(&rows_json).expect("rows json should parse");
            self.events.lock().unwrap().push(TestEvent::Result(rows));
        }

        fn on_error(&self, message: String) {
            self.events.lock().unwrap().push(TestEvent::Error(message));
        }
    }

    impl TestListener {
        fn capture() -> (Arc<Self>, Arc<Mutex<Vec<TestEvent>>>) {
            let events = Arc::new(Mutex::new(Vec::new()));
            (
                Arc::new(Self {
                    events: Arc::clone(&events),
                }),
                events,
            )
        }
    }

    fn next_event(events: &Arc<Mutex<Vec<TestEvent>>>, index: usize) -> TestEvent {
        let deadline = std::time::Instant::now() + Duration::from_secs(1);
        loop {
            if let Some(event) = events.lock().unwrap().get(index).cloned() {
                return event;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "timed out waiting for event {index}"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    fn wait_for_stable_event_count(
        events: &Arc<Mutex<Vec<TestEvent>>>,
        stable_for: Duration,
    ) -> usize {
        let mut last_len = events.lock().unwrap().len();
        loop {
            std::thread::sleep(stable_for);
            let len = events.lock().unwrap().len();
            if len == last_len {
                return len;
            }
            last_len = len;
        }
    }

    fn new_bridge() -> (tempfile::TempDir, MobileDbBridge) {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("app.db");
        let bridge = MobileDbBridge::open(db_path.to_string_lossy().into_owned()).unwrap();
        (dir, bridge)
    }

    const REENTRANT_SUBSCRIBE_CHILD_ENV: &str = "MOBILE_BRIDGE_REENTRANT_SUBSCRIBE_CHILD";

    #[test]
    fn execute_roundtrips_rows() {
        let (_dir, bridge) = new_bridge();

        bridge
            .execute(
                "INSERT INTO templates (id, title) VALUES (?, ?)".to_string(),
                r#"["template-1","Weekly"]"#.to_string(),
            )
            .unwrap();

        let rows_json = bridge
            .execute(
                "SELECT id, title FROM templates ORDER BY id".to_string(),
                "[]".to_string(),
            )
            .unwrap();
        let rows: Vec<serde_json::Value> = serde_json::from_str(&rows_json).unwrap();

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["id"], "template-1");
        assert_eq!(rows[0]["title"], "Weekly");
    }

    #[test]
    fn execute_proxy_roundtrips_positional_rows() {
        let (_dir, bridge) = new_bridge();

        bridge
            .execute(
                "INSERT INTO templates (id, title) VALUES (?, ?)".to_string(),
                r#"["template-1","Weekly"]"#.to_string(),
            )
            .unwrap();

        let result_json = bridge
            .execute_proxy(
                "SELECT id, title FROM templates ORDER BY id".to_string(),
                "[]".to_string(),
                "all".to_string(),
            )
            .unwrap();
        let result: hypr_db_execute::ProxyQueryResult = serde_json::from_str(&result_json).unwrap();

        assert_eq!(
            result.rows,
            vec![serde_json::json!(["template-1", "Weekly"])]
        );
    }

    #[test]
    fn subscribe_reruns_after_write() {
        let (_dir, bridge) = new_bridge();
        let (listener, events) = TestListener::capture();

        let subscription_id = bridge
            .subscribe(
                "SELECT id, title FROM templates ORDER BY id".to_string(),
                "[]".to_string(),
                listener,
            )
            .unwrap();

        let initial = next_event(&events, 0);
        assert_eq!(initial, TestEvent::Result(Vec::new()));

        bridge
            .execute(
                "INSERT INTO templates (id, title) VALUES (?, ?)".to_string(),
                r#"["template-live","Retro"]"#.to_string(),
            )
            .unwrap();

        let refresh = next_event(&events, 1);
        let TestEvent::Result(rows) = refresh else {
            panic!("expected result event");
        };
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["id"], "template-live");

        bridge.unsubscribe(subscription_id).unwrap();
    }

    #[test]
    fn subscribe_listener_can_reenter_bridge_without_deadlock() {
        if std::env::var_os(REENTRANT_SUBSCRIBE_CHILD_ENV).is_some() {
            #[derive(Clone)]
            struct ReentrantListener {
                bridge: std::sync::Weak<MobileDbBridge>,
                callback_completed: Arc<AtomicBool>,
            }

            impl QueryEventListener for ReentrantListener {
                fn on_result(&self, _rows_json: String) {
                    let bridge = self
                        .bridge
                        .upgrade()
                        .expect("bridge should be alive during callback");
                    // `execute` takes the state lock, so this deadlocks if the
                    // callback runs while the bridge holds it. Own thread: the
                    // callback runs inside the bridge's `block_on`.
                    std::thread::spawn(move || {
                        bridge
                            .execute("SELECT 1".to_string(), "[]".to_string())
                            .unwrap();
                    })
                    .join()
                    .unwrap();
                    self.callback_completed.store(true, Ordering::SeqCst);
                }

                fn on_error(&self, message: String) {
                    panic!("unexpected error callback: {message}");
                }
            }

            let (_dir, bridge) = new_bridge();
            let bridge = Arc::new(bridge);
            let callback_completed = Arc::new(AtomicBool::new(false));
            let listener = Arc::new(ReentrantListener {
                bridge: Arc::downgrade(&bridge),
                callback_completed: Arc::clone(&callback_completed),
            });

            let subscription_id = bridge
                .subscribe(
                    "SELECT id, title FROM templates ORDER BY id".to_string(),
                    "[]".to_string(),
                    listener,
                )
                .unwrap();
            assert!(callback_completed.load(Ordering::SeqCst));
            bridge.unsubscribe(subscription_id).unwrap();
            return;
        }

        let current_exe = std::env::current_exe().unwrap();
        let mut child = Command::new(current_exe)
            .arg("--exact")
            .arg("tests::subscribe_listener_can_reenter_bridge_without_deadlock")
            .arg("--nocapture")
            .env(REENTRANT_SUBSCRIBE_CHILD_ENV, "1")
            .spawn()
            .unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(5);

        loop {
            if let Some(status) = child.try_wait().unwrap() {
                assert!(status.success(), "child test failed with status {status}");
                break;
            }

            if std::time::Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                panic!("child test timed out; subscribe listener re-entry likely deadlocked");
            }

            std::thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn unsubscribe_stops_future_events() {
        let (_dir, bridge) = new_bridge();
        let (listener, events) = TestListener::capture();

        let subscription_id = bridge
            .subscribe(
                "SELECT id, title FROM templates ORDER BY id".to_string(),
                "[]".to_string(),
                listener,
            )
            .unwrap();

        next_event(&events, 0);
        bridge.unsubscribe(subscription_id).unwrap();

        bridge
            .execute(
                "INSERT INTO templates (id, title) VALUES (?, ?)".to_string(),
                r#"["template-after-unsub","Standup"]"#.to_string(),
            )
            .unwrap();

        let count = wait_for_stable_event_count(&events, Duration::from_millis(100));
        assert_eq!(count, 1);
    }

    #[test]
    fn close_rejects_future_calls() {
        let (_dir, bridge) = new_bridge();

        bridge.close().unwrap();

        assert!(matches!(
            bridge.execute("SELECT 1".to_string(), "[]".to_string()),
            Err(BridgeError::Closed)
        ));
    }

    #[test]
    fn invalid_params_shape_is_rejected() {
        let (_dir, bridge) = new_bridge();

        let error = bridge
            .execute("SELECT 1".to_string(), "{}".to_string())
            .unwrap_err();

        assert!(matches!(error, BridgeError::ParamsMustBeArray));
    }
}

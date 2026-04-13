use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use hypr_db_core2::{Db3, DbOpenOptions, DbStorage, MigrationFailurePolicy};
use hypr_db_watch::{TableDeps, WatchId, extract_tables};
use sqlx::{Column, Row, TypeInfo, ValueRef};
use tauri::Manager;
use tauri::ipc::Channel;
use tokio::sync::watch;

use crate::{Error, QueryEvent, Result};

struct Subscription {
    id: String,
    sql: String,
    params: Vec<serde_json::Value>,
    channel: Channel<QueryEvent>,
}

#[derive(Default)]
struct Inner {
    deps: TableDeps,
    ids: std::collections::HashMap<String, WatchId>,
    subscriptions: std::collections::HashMap<WatchId, Subscription>,
}

pub struct DbRuntime {
    db: Arc<Db3>,
    inner: Arc<tokio::sync::Mutex<Inner>>,
    shutdown_tx: watch::Sender<bool>,
    dispatcher: Mutex<Option<tokio::task::JoinHandle<()>>>,
}

impl DbRuntime {
    pub fn new(db: Db3) -> Self {
        let db = Arc::new(db);
        let inner = Arc::new(tokio::sync::Mutex::new(Inner::default()));
        let (shutdown_tx, mut shutdown_rx) = watch::channel(false);
        let mut change_rx = db.subscribe_table_changes();
        let dispatcher_inner = Arc::clone(&inner);
        let dispatcher_db = Arc::clone(&db);

        let dispatcher = tokio::spawn(async move {
            loop {
                tokio::select! {
                    changed = shutdown_rx.changed() => {
                        if changed.is_err() || *shutdown_rx.borrow() {
                            break;
                        }
                    }
                    change = change_rx.recv() => {
                        let Ok(first_change) = change else {
                            break;
                        };

                        let mut changed_tables = HashSet::from([first_change.table]);
                        while let Ok(next_change) = change_rx.try_recv() {
                            changed_tables.insert(next_change.table);
                        }

                        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

                        let jobs = collect_jobs(&dispatcher_inner, &changed_tables).await;
                        if jobs.is_empty() {
                            continue;
                        }

                        let mut stale = Vec::new();
                        for job in jobs {
                            let event = run_query(&dispatcher_db, &job.sql, &job.params)
                                .await
                                .map(QueryEvent::Result)
                                .unwrap_or_else(|error| QueryEvent::Error(error.to_string()));

                            if job.channel.send(event).is_err() {
                                stale.push(job.watch_id);
                            }
                        }

                        if !stale.is_empty() {
                            remove_stale(&dispatcher_inner, &stale).await;
                        }
                    }
                }
            }
        });

        Self {
            db,
            inner,
            shutdown_tx,
            dispatcher: Mutex::new(Some(dispatcher)),
        }
    }

    pub fn pool(&self) -> &sqlx::SqlitePool {
        self.db.pool()
    }

    pub async fn execute(
        &self,
        sql: String,
        params: Vec<serde_json::Value>,
    ) -> Result<Vec<serde_json::Value>> {
        run_query(&self.db, &sql, &params).await.map_err(Into::into)
    }

    pub async fn subscribe(
        &self,
        sql: String,
        params: Vec<serde_json::Value>,
        channel: Channel<QueryEvent>,
    ) -> Result<String> {
        let subscription_id = uuid::Uuid::new_v4().to_string();
        let watch_tables = extract_tables(self.db.pool(), &sql)
            .await
            .unwrap_or_default();

        let watch_id = {
            let mut inner = self.inner.lock().await;
            let watch_id = inner.deps.register(watch_tables);
            inner.ids.insert(subscription_id.clone(), watch_id);
            inner.subscriptions.insert(
                watch_id,
                Subscription {
                    id: subscription_id.clone(),
                    sql: sql.clone(),
                    params: params.clone(),
                    channel: channel.clone(),
                },
            );
            watch_id
        };

        let event = match run_query(&self.db, &sql, &params).await {
            Ok(rows) => QueryEvent::Result(rows),
            Err(error) => QueryEvent::Error(error.to_string()),
        };

        if let Err(error) = channel.send(event) {
            self.remove_watch(watch_id).await;
            return Err(Error::Channel(error.to_string()));
        }

        Ok(subscription_id)
    }

    pub async fn unsubscribe(&self, subscription_id: &str) -> Result<()> {
        let watch_id = {
            let mut inner = self.inner.lock().await;
            inner
                .ids
                .remove(subscription_id)
                .ok_or_else(|| Error::SubscriptionNotFound(subscription_id.to_string()))?
        };
        self.remove_watch(watch_id).await;
        Ok(())
    }

    async fn remove_watch(&self, watch_id: WatchId) {
        let mut inner = self.inner.lock().await;
        if let Some(subscription) = inner.subscriptions.remove(&watch_id) {
            inner.ids.remove(&subscription.id);
        }
        inner.deps.unregister(watch_id);
    }
}

impl Drop for DbRuntime {
    fn drop(&mut self) {
        let _ = self.shutdown_tx.send(true);
        if let Some(dispatcher) = self.dispatcher.lock().unwrap().take() {
            dispatcher.abort();
        }
    }
}

#[derive(Clone)]
struct RefreshJob {
    watch_id: WatchId,
    sql: String,
    params: Vec<serde_json::Value>,
    channel: Channel<QueryEvent>,
}

async fn collect_jobs(
    inner: &Arc<tokio::sync::Mutex<Inner>>,
    changed_tables: &HashSet<String>,
) -> Vec<RefreshJob> {
    let changed_refs = changed_tables
        .iter()
        .map(std::string::String::as_str)
        .collect::<Vec<_>>();

    let guard = inner.lock().await;
    guard
        .deps
        .affected(&changed_refs)
        .into_iter()
        .filter_map(|watch_id| {
            guard
                .subscriptions
                .get(&watch_id)
                .map(|subscription| RefreshJob {
                    watch_id,
                    sql: subscription.sql.clone(),
                    params: subscription.params.clone(),
                    channel: subscription.channel.clone(),
                })
        })
        .collect()
}

async fn remove_stale(inner: &Arc<tokio::sync::Mutex<Inner>>, stale: &[WatchId]) {
    let mut guard = inner.lock().await;
    for watch_id in stale {
        if let Some(subscription) = guard.subscriptions.remove(watch_id) {
            guard.ids.remove(&subscription.id);
        }
        guard.deps.unregister(*watch_id);
    }
}

async fn run_query(
    db: &Db3,
    sql: &str,
    params: &[serde_json::Value],
) -> std::result::Result<Vec<serde_json::Value>, sqlx::Error> {
    let mut query = sqlx::query(sql);
    for param in params {
        query = match param {
            serde_json::Value::Null => query.bind(None::<String>),
            serde_json::Value::Bool(value) => query.bind(*value),
            serde_json::Value::Number(value) => {
                if let Some(integer) = value.as_i64() {
                    query.bind(integer)
                } else {
                    query.bind(value.as_f64().unwrap_or_default())
                }
            }
            serde_json::Value::String(value) => query.bind(value.clone()),
            other => query.bind(other.to_string()),
        };
    }

    let rows = query.fetch_all(db.pool().as_ref()).await?;
    Ok(rows.iter().map(row_to_json).collect())
}

fn row_to_json(row: &sqlx::sqlite::SqliteRow) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    for (index, column) in row.columns().iter().enumerate() {
        let value = match row.try_get_raw(index) {
            Ok(raw) if !raw.is_null() => match raw.type_info().name() {
                "TEXT" => row
                    .get::<Option<String>, _>(index)
                    .map(serde_json::Value::String)
                    .unwrap_or(serde_json::Value::Null),
                "INTEGER" | "INT" | "BOOLEAN" => row
                    .get::<Option<i64>, _>(index)
                    .map(serde_json::Value::from)
                    .unwrap_or(serde_json::Value::Null),
                "REAL" => row
                    .get::<Option<f64>, _>(index)
                    .map(serde_json::Value::from)
                    .unwrap_or(serde_json::Value::Null),
                "BLOB" => row
                    .get::<Option<Vec<u8>>, _>(index)
                    .map(serde_json::Value::from)
                    .unwrap_or(serde_json::Value::Null),
                _ => row
                    .get::<Option<String>, _>(index)
                    .map(serde_json::Value::String)
                    .unwrap_or(serde_json::Value::Null),
            },
            _ => serde_json::Value::Null,
        };
        map.insert(column.name().to_string(), value);
    }

    serde_json::Value::Object(map)
}

pub fn open_app_db<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> Result<Db3> {
    let db_path = app
        .path()
        .app_data_dir()
        .map_err(|error| Error::Io(std::io::Error::other(error.to_string())))?
        .join("app.db");

    let db = std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("failed to create db init runtime");
        runtime.block_on(Db3::open_with_migrate(
            DbOpenOptions {
                storage: DbStorage::Local(&db_path),
                cloudsync: false,
                journal_mode_wal: true,
                foreign_keys: true,
                max_connections: Some(4),
                migration_failure_policy: MigrationFailurePolicy::Fail,
            },
            |pool| Box::pin(hypr_db_app::migrate(pool)),
        ))
    })
    .join()
    .expect("db init thread panicked")?;

    Ok(db)
}

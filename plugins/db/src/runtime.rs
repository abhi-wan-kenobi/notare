use std::path::Path;

use hypr_db_core::{Db, DbOpenOptions, DbStorage};
use hypr_db_execute::{DbExecutor, ProxyQueryMethod, ProxyQueryResult};
use hypr_db_reactive::{LiveQueryRuntime, QueryEventSink, SubscriptionRegistration};
use tauri::ipc::Channel;

use crate::{QueryEvent, Result};

#[derive(Clone)]
pub struct QueryEventChannel(Channel<QueryEvent>);

impl QueryEventChannel {
    pub fn new(channel: Channel<QueryEvent>) -> Self {
        Self(channel)
    }
}

impl QueryEventSink for QueryEventChannel {
    fn send_result(&self, rows: Vec<serde_json::Value>) -> std::result::Result<(), String> {
        self.0
            .send(QueryEvent::Result(rows))
            .map_err(|error| error.to_string())
    }

    fn send_error(&self, error: String) -> std::result::Result<(), String> {
        self.0
            .send(QueryEvent::Error(error))
            .map_err(|error| error.to_string())
    }
}

pub struct PluginDbRuntime {
    executor: DbExecutor,
    live_query_runtime: LiveQueryRuntime<QueryEventChannel>,
}

impl PluginDbRuntime {
    pub fn new(db: std::sync::Arc<Db>) -> Self {
        Self {
            executor: DbExecutor::new(std::sync::Arc::clone(&db)),
            live_query_runtime: LiveQueryRuntime::new(db),
        }
    }

    pub async fn execute(
        &self,
        sql: String,
        params: Vec<serde_json::Value>,
    ) -> hypr_db_execute::Result<Vec<serde_json::Value>> {
        self.executor.execute(sql, params).await
    }

    pub async fn execute_proxy(
        &self,
        sql: String,
        params: Vec<serde_json::Value>,
        method: ProxyQueryMethod,
    ) -> hypr_db_execute::Result<ProxyQueryResult> {
        self.executor.execute_proxy(sql, params, method).await
    }

    pub async fn subscribe(
        &self,
        sql: String,
        params: Vec<serde_json::Value>,
        sink: QueryEventChannel,
    ) -> hypr_db_reactive::Result<SubscriptionRegistration> {
        self.live_query_runtime.subscribe(sql, params, sink).await
    }

    pub async fn unsubscribe(&self, subscription_id: &str) -> hypr_db_reactive::Result<()> {
        self.live_query_runtime.unsubscribe(subscription_id).await
    }
}

pub async fn open_app_db(db_path: Option<&Path>) -> Result<Db> {
    let storage = match db_path {
        Some(path) => DbStorage::Local(path),
        None => DbStorage::Memory,
    };

    let db = Db::open(DbOpenOptions {
        storage,
        cloudsync_enabled: false,
        journal_mode_wal: true,
        foreign_keys: true,
        max_connections: Some(4),
    })
    .await?;

    hypr_db_migrate::migrate(&db, hypr_db_app::schema()).await?;

    Ok(db)
}

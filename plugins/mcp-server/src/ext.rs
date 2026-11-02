use std::sync::Arc;
use tokio::sync::{RwLock, watch};

use tauri_plugin_store2::{ScopedStore, Store2PluginExt};

use crate::error::{Error, Result};
use crate::http::run_server;
use crate::server::NotareMcpServer;
use crate::types::*;

pub struct McpServerState {
    pub running: bool,
    pub port: u16,
    pub last_error: Option<String>,
    pub shutdown_tx: Option<watch::Sender<bool>>,
}

pub struct McpServerManager {
    pub state: Arc<RwLock<McpServerState>>,
    pub pool: sqlx::SqlitePool,
    pub app_handle: tauri::AppHandle,
}

impl McpServerManager {
    pub fn new(pool: sqlx::SqlitePool, app_handle: tauri::AppHandle) -> Self {
        Self {
            state: Arc::new(RwLock::new(McpServerState {
                running: false,
                port: DEFAULT_PORT,
                last_error: None,
                shutdown_tx: None,
            })),
            pool,
            app_handle,
        }
    }

    fn scoped(&self) -> Result<ScopedStore<tauri::Wry, StoreKey>> {
        Ok(self
            .app_handle
            .store2()
            .scoped_store::<StoreKey>(STORE_SCOPE)?)
    }

    pub fn get_or_create_token(&self) -> Result<String> {
        if let Some(token) = tauri_plugin_store2::secrets::get_secret_blocking(
            &self.app_handle,
            SECRET_SCOPE,
            SECRET_KEY,
        )
        .map_err(Error::Keyring)?
            && !token.is_empty() {
                return Ok(token);
            }

        let new_token = format!(
            "ntr_mcp_{}{}",
            uuid::Uuid::new_v4().simple(),
            uuid::Uuid::new_v4().simple()
        );
        tauri_plugin_store2::secrets::set_secret_blocking(
            &self.app_handle,
            SECRET_SCOPE,
            SECRET_KEY,
            &new_token,
        )
        .map_err(Error::Keyring)?;

        Ok(new_token)
    }

    pub fn regenerate_token(&self) -> Result<String> {
        let new_token = format!(
            "ntr_mcp_{}{}",
            uuid::Uuid::new_v4().simple(),
            uuid::Uuid::new_v4().simple()
        );
        tauri_plugin_store2::secrets::set_secret_blocking(
            &self.app_handle,
            SECRET_SCOPE,
            SECRET_KEY,
            &new_token,
        )
        .map_err(Error::Keyring)?;

        Ok(new_token)
    }

    pub fn get_settings(&self) -> Result<McpServerSettings> {
        let store = self.scoped()?;
        let enabled = store.get::<bool>(StoreKey::Enabled)?.unwrap_or(false);
        let port = store.get::<u16>(StoreKey::Port)?.unwrap_or(DEFAULT_PORT);
        let bearer_token = self.get_or_create_token()?;

        Ok(McpServerSettings {
            enabled,
            port,
            bearer_token,
        })
    }

    pub async fn set_settings(&self, input: McpServerSettingsInput) -> Result<McpServerSettings> {
        let store = self.scoped()?;
        store.set(StoreKey::Enabled, input.enabled)?;
        store.set(StoreKey::Port, input.port)?;
        store.save()?;

        if input.enabled {
            self.restart(input.port).await?;
        } else {
            self.stop().await;
        }

        self.get_settings()
    }

    pub async fn get_status(&self) -> McpServerStatus {
        let guard = self.state.read().await;
        McpServerStatus {
            running: guard.running,
            port: guard.port,
            error: guard.last_error.clone(),
        }
    }

    pub async fn stop(&self) {
        let mut guard = self.state.write().await;
        if let Some(tx) = guard.shutdown_tx.take() {
            let _ = tx.send(true);
        }
        guard.running = false;
        guard.last_error = None;
    }

    pub async fn restart(&self, port: u16) -> Result<()> {
        self.stop().await;

        let token = self.get_or_create_token()?;
        let (tx, rx) = watch::channel(false);

        let server = NotareMcpServer::new(self.pool.clone(), Some(self.app_handle.clone()));
        let state_clone = self.state.clone();

        let listener_check = tokio::net::TcpListener::bind(("127.0.0.1", port)).await;
        if let Err(e) = listener_check {
            let err = if e.kind() == std::io::ErrorKind::AddrInUse {
                Error::PortInUse(port)
            } else {
                Error::Server(e.to_string())
            };
            let mut guard = state_clone.write().await;
            guard.running = false;
            guard.port = port;
            guard.last_error = Some(err.to_string());
            return Err(err);
        }
        drop(listener_check);

        {
            let mut guard = self.state.write().await;
            guard.running = true;
            guard.port = port;
            guard.last_error = None;
            guard.shutdown_tx = Some(tx);
        }

        tokio::spawn(async move {
            if let Err(e) = run_server(server, port, token, rx).await {
                tracing::error!("MCP server exited with error: {e}");
                let mut guard = state_clone.write().await;
                guard.running = false;
                guard.last_error = Some(e.to_string());
            }
        });

        Ok(())
    }
}

use std::sync::Arc;
use tokio::sync::{RwLock, watch};

use tauri_plugin_store2::{ScopedStore, Store2PluginExt};

use crate::error::{Error, Result};
use crate::http::run_server;
use crate::server::NotareMcpServer;
use crate::types::*;

pub fn validate_port(port: u16, currently_running_on: Option<u16>) -> Result<()> {
    if port == 0 {
        return Err(Error::InvalidPort(port));
    }
    if port < 1024 && currently_running_on != Some(port) {
        return Err(Error::InvalidPort(port));
    }
    Ok(())
}

pub struct McpServerState {
    pub running: bool,
    pub port: u16,
    pub last_error: Option<String>,
    pub shutdown_tx: Option<watch::Sender<bool>>,
    pub task_handle: Option<tokio::task::JoinHandle<()>>,
}

#[derive(Clone)]
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
                task_handle: None,
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
            && !token.trim().is_empty()
        {
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

    /// `(enabled, port)` from the plain store only — no keyring. Used at app
    /// startup so a disabled server never triggers a keychain prompt.
    pub fn autostart_config(&self) -> Result<(bool, u16)> {
        let store = self.scoped()?;
        let enabled = store.get::<bool>(StoreKey::Enabled)?.unwrap_or(false);
        let port = store.get::<u16>(StoreKey::Port)?.unwrap_or(DEFAULT_PORT);
        Ok((enabled, if port == 0 { DEFAULT_PORT } else { port }))
    }

    pub fn get_settings(&self) -> Result<McpServerSettings> {
        let store = self.scoped()?;
        let enabled = store.get::<bool>(StoreKey::Enabled)?.unwrap_or(false);
        let port = store.get::<u16>(StoreKey::Port)?.unwrap_or(DEFAULT_PORT);
        let port = if port == 0 { DEFAULT_PORT } else { port };
        let bearer_token = self.get_or_create_token()?;

        Ok(McpServerSettings {
            enabled,
            port,
            bearer_token,
        })
    }

    pub async fn set_settings(&self, input: McpServerSettingsInput) -> Result<McpServerSettings> {
        let current_running_port = {
            let guard = self.state.read().await;
            if guard.running {
                Some(guard.port)
            } else {
                None
            }
        };
        validate_port(input.port, current_running_port)?;

        if input.enabled {
            self.restart(input.port).await?;
        } else {
            self.stop().await;
        }

        let store = self.scoped()?;
        store.set(StoreKey::Enabled, input.enabled)?;
        store.set(StoreKey::Port, input.port)?;
        store.save()?;

        self.get_settings()
    }

    pub async fn regenerate_token_and_restart(&self) -> Result<String> {
        let (running, port) = {
            let guard = self.state.read().await;
            (guard.running, guard.port)
        };
        let token = self.regenerate_token()?;
        if running {
            self.restart(port).await?;
        }
        Ok(token)
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
        let (tx, handle) = {
            let mut guard = self.state.write().await;
            guard.running = false;
            guard.last_error = None;
            (guard.shutdown_tx.take(), guard.task_handle.take())
        };
        if let Some(tx) = tx {
            let _ = tx.send(true);
        }
        if let Some(handle) = handle {
            let _ = handle.await;
        }
    }

    pub async fn restart(&self, port: u16) -> Result<()> {
        let current_running_port = {
            let guard = self.state.read().await;
            if guard.running {
                Some(guard.port)
            } else {
                None
            }
        };
        validate_port(port, current_running_port)?;

        self.stop().await;

        let token = self.get_or_create_token()?;
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();

        let server = NotareMcpServer::new(self.pool.clone(), Some(self.app_handle.clone()));
        let state_clone = self.state.clone();

        let handle = tokio::spawn(async move {
            let res = run_server(server, port, token, shutdown_rx, Some(ready_tx)).await;
            let mut guard = state_clone.write().await;
            guard.running = false;
            if let Err(e) = res {
                tracing::error!("MCP server exited with error: {e}");
                guard.last_error = Some(e.to_string());
            }
        });

        match ready_rx.await {
            Ok(Ok(())) => {
                let mut guard = self.state.write().await;
                guard.running = true;
                guard.port = port;
                guard.last_error = None;
                guard.shutdown_tx = Some(shutdown_tx);
                guard.task_handle = Some(handle);
                Ok(())
            }
            Ok(Err(e)) => {
                let _ = handle.await;
                let mut guard = self.state.write().await;
                guard.running = false;
                guard.port = port;
                guard.last_error = Some(e.to_string());
                Err(e)
            }
            Err(_) => {
                let _ = handle.await;
                let mut guard = self.state.write().await;
                guard.running = false;
                guard.port = port;
                let err = Error::Server("MCP server task terminated during startup".to_string());
                guard.last_error = Some(err.to_string());
                Err(err)
            }
        }
    }
}

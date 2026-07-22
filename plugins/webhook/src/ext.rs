use tauri_plugin_store2::{ScopedStore, Store2PluginExt};

use crate::error::{Error, Result};
use crate::types::{
    DELIVERY_LOG_CAP, DeliveryRecord, SECRET_KEY, SECRET_SCOPE, STORE_SCOPE, StoreKey,
    WebhookEvents, WebhookSettings,
};

pub struct Webhook<'a, R: tauri::Runtime, M: tauri::Manager<R>> {
    manager: &'a M,
    _runtime: std::marker::PhantomData<fn() -> R>,
}

impl<'a, R: tauri::Runtime, M: tauri::Manager<R>> Webhook<'a, R, M> {
    fn scoped(&self) -> Result<ScopedStore<R, StoreKey>> {
        Ok(self
            .manager
            .store2()
            .scoped_store::<StoreKey>(STORE_SCOPE)?)
    }

    /// Current user-facing settings. Never includes the secret value — only a
    /// `has_secret` presence flag.
    pub fn settings(&self) -> Result<WebhookSettings> {
        let store = self.scoped()?;
        let endpoint_url = store
            .get::<String>(StoreKey::EndpointUrl)?
            .unwrap_or_default();
        let enabled = store.get::<bool>(StoreKey::Enabled)?.unwrap_or(false);
        let events = WebhookEvents {
            action_items_updated: store
                .get::<bool>(StoreKey::EventActionItemsUpdated)?
                .unwrap_or(false),
            session_enhanced: store
                .get::<bool>(StoreKey::EventSessionEnhanced)?
                .unwrap_or(false),
        };
        let has_secret = self.has_secret()?;
        Ok(WebhookSettings {
            endpoint_url,
            enabled,
            events,
            has_secret,
        })
    }

    /// Persist endpoint/enabled/per-event flags. Does NOT touch the secret
    /// (managed separately through the keyring helpers below).
    pub fn set_settings(&self, settings: &WebhookSettings) -> Result<()> {
        let store = self.scoped()?;
        store.set(
            StoreKey::EndpointUrl,
            settings.endpoint_url.trim().to_string(),
        )?;
        store.set(StoreKey::Enabled, settings.enabled)?;
        store.set(
            StoreKey::EventActionItemsUpdated,
            settings.events.action_items_updated,
        )?;
        store.set(
            StoreKey::EventSessionEnhanced,
            settings.events.session_enhanced,
        )?;
        store.save()?;
        Ok(())
    }

    // --- Signing secret (OS keyring; BLOCKING — call from spawn_blocking) ---

    pub fn set_secret(&self, secret: &str) -> Result<()> {
        tauri_plugin_store2::secrets::set_secret_blocking(
            self.manager.app_handle(),
            SECRET_SCOPE,
            SECRET_KEY,
            secret,
        )
        .map_err(Error::Keyring)
    }

    pub fn get_secret(&self) -> Result<Option<String>> {
        tauri_plugin_store2::secrets::get_secret_blocking(
            self.manager.app_handle(),
            SECRET_SCOPE,
            SECRET_KEY,
        )
        .map_err(Error::Keyring)
    }

    pub fn clear_secret(&self) -> Result<()> {
        tauri_plugin_store2::secrets::delete_secret_blocking(
            self.manager.app_handle(),
            SECRET_SCOPE,
            SECRET_KEY,
        )
        .map_err(Error::Keyring)
    }

    pub fn has_secret(&self) -> Result<bool> {
        Ok(self.get_secret()?.map(|s| !s.is_empty()).unwrap_or(false))
    }

    // --- In-memory delivery log (bounded ring buffer) ---

    pub fn record_delivery(&self, record: DeliveryRecord) {
        if let Some(state) = self.manager.try_state::<crate::State>() {
            if let Ok(mut log) = state.log.lock() {
                log.push_front(record);
                log.truncate(DELIVERY_LOG_CAP);
            }
        }
    }

    pub fn recent_deliveries(&self) -> Vec<DeliveryRecord> {
        self.manager
            .try_state::<crate::State>()
            .map(|state| {
                state
                    .log
                    .lock()
                    .map(|log| log.iter().cloned().collect())
                    .unwrap_or_default()
            })
            .unwrap_or_default()
    }

    pub fn http_client(&self) -> Option<reqwest::Client> {
        self.manager
            .try_state::<crate::State>()
            .map(|state| state.client.clone())
    }
}

pub trait WebhookPluginExt<R: tauri::Runtime> {
    fn webhook(&self) -> Webhook<'_, R, Self>
    where
        Self: tauri::Manager<R> + Sized;
}

impl<R: tauri::Runtime, T: tauri::Manager<R>> WebhookPluginExt<R> for T {
    fn webhook(&self) -> Webhook<'_, R, Self>
    where
        Self: Sized,
    {
        Webhook {
            manager: self,
            _runtime: std::marker::PhantomData,
        }
    }
}

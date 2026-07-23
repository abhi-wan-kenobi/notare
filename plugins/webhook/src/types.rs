use serde::{Deserialize, Serialize};

/// Store scope (store2 namespace) for webhook settings.
pub const STORE_SCOPE: &str = "webhook";
/// Keyring scope for the per-webhook signing secret.
pub const SECRET_SCOPE: &str = "webhook";
/// Keyring key for the per-webhook signing secret.
pub const SECRET_KEY: &str = "signing_secret";

/// Canonical event-type strings. `send_webhook` is a no-op for any
/// `event_type` not represented here (and for events not opted-in).
pub const EVENT_ACTION_ITEMS_UPDATED: &str = "action_items.updated";
pub const EVENT_SESSION_ENHANCED: &str = "session.enhanced";

/// Maximum number of delivery records kept in the in-memory log.
pub const DELIVERY_LOG_CAP: usize = 50;

/// Typed keys for the webhook scoped store.
#[derive(PartialEq, Eq, Hash, strum::Display)]
pub enum StoreKey {
    #[strum(serialize = "endpoint_url")]
    EndpointUrl,
    #[strum(serialize = "enabled")]
    Enabled,
    #[strum(serialize = "event_action_items_updated")]
    EventActionItemsUpdated,
    #[strum(serialize = "event_session_enhanced")]
    EventSessionEnhanced,
}

impl tauri_plugin_store2::ScopedStoreKey for StoreKey {}

/// Per-event opt-in flags. A webhook only fires for an event whose flag is
/// `true`; everything is off by default (opt-in outbound integration).
#[derive(Debug, Clone, Default, Serialize, Deserialize, specta::Type)]
pub struct WebhookEvents {
    #[serde(default)]
    pub action_items_updated: bool,
    #[serde(default)]
    pub session_enhanced: bool,
}

impl WebhookEvents {
    /// Whether the given canonical event-type is opted-in.
    pub fn is_enabled(&self, event_type: &str) -> bool {
        match event_type {
            EVENT_ACTION_ITEMS_UPDATED => self.action_items_updated,
            EVENT_SESSION_ENHANCED => self.session_enhanced,
            _ => false,
        }
    }
}

/// User-facing webhook configuration. The signing secret is intentionally NOT
/// part of this struct — it lives only in the OS keyring and is never returned
/// to the frontend. `has_secret` reports presence, not the value.
#[derive(Debug, Clone, Default, Serialize, Deserialize, specta::Type)]
pub struct WebhookSettings {
    /// Destination endpoint URL. Empty string means "not configured".
    #[serde(default)]
    pub endpoint_url: String,
    /// Master on/off switch. When false, `send_webhook` is always a no-op.
    #[serde(default)]
    pub enabled: bool,
    /// Per-event opt-in flags.
    #[serde(default)]
    pub events: WebhookEvents,
    /// Whether a signing secret is currently stored (read-only; never the value).
    #[serde(default)]
    pub has_secret: bool,
}

/// One record in the delivery log.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct DeliveryRecord {
    /// Unique delivery id (also sent as `X-Notare-Delivery`).
    pub id: String,
    /// Event type this delivery carried.
    pub event_type: String,
    /// ISO-8601 (RFC 3339) timestamp of the delivery attempt completion.
    pub timestamp: String,
    /// Final HTTP status code, if a response was received.
    pub status_code: Option<u16>,
    /// Error string when delivery failed without a usable response.
    pub error: Option<String>,
    /// Number of attempts made (1..=max_attempts).
    pub attempts: u32,
    /// Whether the delivery ultimately succeeded (2xx response).
    pub success: bool,
}

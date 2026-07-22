use crate::WebhookPluginExt;
use crate::delivery::{RetryPolicy, WebhookEnvelope, deliver};
use crate::types::{DeliveryRecord, WebhookSettings};

/// Read the current webhook settings (endpoint, enabled, per-event flags, and a
/// `has_secret` presence flag — never the secret value).
#[tauri::command]
#[specta::specta]
pub async fn get_settings<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<WebhookSettings, String> {
    // Keyring read is blocking; hop off the async runtime.
    tauri::async_runtime::spawn_blocking(move || {
        app.webhook().settings().map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Persist endpoint/enabled/per-event flags. Does NOT change the secret.
#[tauri::command]
#[specta::specta]
pub async fn set_settings<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    settings: WebhookSettings,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        app.webhook()
            .set_settings(&settings)
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Store (or replace) the HMAC signing secret in the OS keyring.
#[tauri::command]
#[specta::specta]
pub async fn set_secret<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    secret: String,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        app.webhook().set_secret(&secret).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Delete the stored signing secret.
#[tauri::command]
#[specta::specta]
pub async fn clear_secret<R: tauri::Runtime>(app: tauri::AppHandle<R>) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        app.webhook().clear_secret().map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Recent delivery attempts (newest first), for the settings panel's log view.
#[tauri::command]
#[specta::specta]
pub async fn recent_deliveries<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<Vec<DeliveryRecord>, String> {
    Ok(app.webhook().recent_deliveries())
}

/// Send a webhook for `event_type` carrying `payload`.
///
/// No-op (returns `Ok(None)`) when: the integration is disabled, no endpoint is
/// configured, no secret is set, or the webhook is not opted-in to
/// `event_type`. On an attempted delivery it returns the `DeliveryRecord` and
/// appends it to the in-memory log.
#[tauri::command]
#[specta::specta]
pub async fn send_webhook<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    event_type: String,
    payload: serde_json::Value,
) -> Result<Option<DeliveryRecord>, String> {
    dispatch(app, event_type, payload, RetryPolicy::default()).await
}

/// Send a synthetic `webhook.test` event so the user can validate their
/// endpoint + secret from the settings panel. Ignores per-event opt-in (but
/// still requires an endpoint, a secret, and the master switch enabled).
#[tauri::command]
#[specta::specta]
pub async fn test_webhook<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<Option<DeliveryRecord>, String> {
    let settings = load_settings(&app).await?;
    if !settings.enabled || settings.endpoint_url.is_empty() {
        return Ok(None);
    }
    let payload = serde_json::json!({ "message": "Notare webhook test event" });
    deliver_now(
        app,
        "webhook.test".to_string(),
        payload,
        RetryPolicy::default(),
    )
    .await
}

// --- internals -----------------------------------------------------------

async fn load_settings<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
) -> Result<WebhookSettings, String> {
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        app.webhook().settings().map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

async fn dispatch<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    event_type: String,
    payload: serde_json::Value,
    policy: RetryPolicy,
) -> Result<Option<DeliveryRecord>, String> {
    let settings = load_settings(&app).await?;

    // Opt-in gate: off unless enabled, configured, and subscribed to this event.
    if !settings.enabled
        || settings.endpoint_url.is_empty()
        || !settings.events.is_enabled(&event_type)
    {
        return Ok(None);
    }

    deliver_now(app, event_type, payload, policy).await
}

async fn deliver_now<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    event_type: String,
    payload: serde_json::Value,
    policy: RetryPolicy,
) -> Result<Option<DeliveryRecord>, String> {
    let settings = load_settings(&app).await?;

    // Secret read is blocking (keyring).
    let secret = {
        let app = app.clone();
        tauri::async_runtime::spawn_blocking(move || {
            app.webhook().get_secret().map_err(|e| e.to_string())
        })
        .await
        .map_err(|e| e.to_string())??
    };
    let Some(secret) = secret.filter(|s| !s.is_empty()) else {
        // No secret => refuse to send unsigned. Treated as not-configured.
        return Ok(None);
    };

    let client = app
        .webhook()
        .http_client()
        .ok_or("webhook state not initialized")?;

    let envelope = WebhookEnvelope {
        id: uuid::Uuid::new_v4().to_string(),
        event_type: event_type.clone(),
        timestamp: chrono::Utc::now().to_rfc3339(),
        data: payload,
    };

    let record = deliver(&client, &settings.endpoint_url, &secret, &envelope, policy).await;
    app.webhook().record_delivery(record.clone());
    Ok(Some(record))
}

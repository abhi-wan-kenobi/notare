//! Direct Microsoft Outlook Calendar integration.
//!
//! Features:
//! - Microsoft identity platform public client (no client secret) using PKCE S256.
//! - Loopback redirect to `http://localhost:<port>`.
//! - Multi-account support: credentials persisted per account at keyring key
//!   `outlook-calendar/oauth/<account_email>`.
//! - Connection IDs formatted as `outlook:<email>`.
//! - Incremental write scope `Calendars.ReadWrite` requested when creating events.
//! - First-party bundled client via `NOTARE_MS_CLIENT_ID` with BYO client ID fallback.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tauri::Manager;
use tauri_plugin_opener::OpenerExt;

use crate::error::Error;

/// Secure-store (OS keyring) location for the credentials blob.
pub const SECRET_SCOPE: &str = "outlook-calendar";
pub const SECRET_KEY_ACCOUNTS: &str = "accounts";
pub const SECRET_KEY_CLIENT: &str = "client";

pub const DEFAULT_READ_SCOPES: &[&str] = hypr_microsoft_oauth::DEFAULT_READ_SCOPES;
pub const WRITE_EVENTS_SCOPE: &str = hypr_microsoft_oauth::WRITE_EVENTS_SCOPE;
pub const DEFAULT_WRITE_SCOPES: &[&str] = hypr_microsoft_oauth::DEFAULT_WRITE_SCOPES;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(300);
const EXPIRY_MARGIN: Duration = Duration::from_secs(60);

/// What gets serialized into each per-account keyring entry (`outlook-calendar/oauth/<email>`).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct StoredOutlookAccount {
    pub email: String,
    pub client_id: String,
    #[serde(default)]
    pub refresh_token: Option<String>,
    #[serde(default)]
    pub granted_scopes: Vec<String>,
}

/// Stored client configuration (BYO client ID).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct StoredOutlookClient {
    pub client_id: String,
}

/// Per-account representation returned to the frontend.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct OutlookConnectedAccount {
    pub email: String,
    pub granted_scopes: Vec<String>,
}

/// Status surfaced to the frontend for Outlook Calendar.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct OutlookAccountStatus {
    /// A client ID is configured (BYO client or bundled client).
    pub has_client: bool,
    /// A bundled first-party client is compiled into this build.
    pub has_bundled_client: bool,
    /// At least one Outlook account is connected.
    pub connected: bool,
    /// The client ID in use.
    pub client_id: Option<String>,
    /// All currently connected Outlook accounts.
    #[serde(default)]
    pub accounts: Vec<OutlookConnectedAccount>,
}

/// Result of importing a client ID.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct OutlookClientImportResult {
    pub client_id: String,
}

#[derive(Clone)]
struct CachedToken {
    access_token: String,
    expires_at: Instant,
}

/// Managed state: in-memory credential + access-token cache so the OS keyring
/// is only touched on first use and on mutations.
pub struct OutlookAuthState {
    pub http: reqwest::Client,
    client: tokio::sync::Mutex<Option<Option<StoredOutlookClient>>>,
    accounts: tokio::sync::Mutex<Option<HashMap<String, StoredOutlookAccount>>>,
    tokens: tokio::sync::Mutex<HashMap<String, CachedToken>>,
    refresh_locks: std::sync::Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>,
}

impl Default for OutlookAuthState {
    fn default() -> Self {
        Self {
            http: reqwest::Client::new(),
            client: tokio::sync::Mutex::new(None),
            accounts: tokio::sync::Mutex::new(None),
            tokens: tokio::sync::Mutex::new(HashMap::new()),
            refresh_locks: std::sync::Mutex::new(HashMap::new()),
        }
    }
}

pub fn auth_error(message: impl std::fmt::Display) -> Error {
    Error::Auth(format!("outlook: {message}"))
}

/// The bundled first-party OAuth client for Outlook.
pub fn bundled_client() -> Option<StoredOutlookClient> {
    let client_id = option_env!("NOTARE_MS_CLIENT_ID").filter(|s| !s.is_empty())?;
    Some(StoredOutlookClient {
        client_id: client_id.to_string(),
    })
}

// === Keyring / Store operations ===

async fn get_secret<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    scope: &str,
    key: &str,
) -> Result<Option<String>, Error> {
    let app = app.clone();
    let scope = scope.to_string();
    let key = key.to_string();
    tauri::async_runtime::spawn_blocking(move || {
        tauri_plugin_store2::secrets::get_secret_blocking(&app, &scope, &key)
    })
    .await
    .map_err(auth_error)?
    .map_err(auth_error)
}

async fn set_secret<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    scope: &str,
    key: &str,
    value: &str,
) -> Result<(), Error> {
    let app = app.clone();
    let scope = scope.to_string();
    let key = key.to_string();
    let value = value.to_string();
    tauri::async_runtime::spawn_blocking(move || {
        tauri_plugin_store2::secrets::set_secret_blocking(&app, &scope, &key, &value)
    })
    .await
    .map_err(auth_error)?
    .map_err(auth_error)
}

async fn delete_secret<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    scope: &str,
    key: &str,
) -> Result<(), Error> {
    let app = app.clone();
    let scope = scope.to_string();
    let key = key.to_string();
    tauri::async_runtime::spawn_blocking(move || {
        tauri_plugin_store2::secrets::delete_secret_blocking(&app, &scope, &key)
    })
    .await
    .map_err(auth_error)?
    .map_err(auth_error)
}

pub async fn read_account_keyring<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    email: &str,
) -> Result<Option<StoredOutlookAccount>, Error> {
    let raw = match get_secret(app, SECRET_SCOPE, &format!("oauth/{email}")).await? {
        Some(raw) => Some(raw),
        None => get_secret(app, "outlook-calendar/oauth", email).await?,
    };

    match raw {
        None => Ok(None),
        Some(raw) => serde_json::from_str(&raw)
            .map(Some)
            .map_err(|e| auth_error(format!("corrupt credentials for {email}: {e}"))),
    }
}

pub async fn write_account_keyring<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    account: &StoredOutlookAccount,
) -> Result<(), Error> {
    let raw = serde_json::to_string(account).map_err(auth_error)?;
    set_secret(app, SECRET_SCOPE, &format!("oauth/{}", account.email), &raw).await?;

    let mut accounts = load_account_emails(app).await?;
    if !accounts.contains(&account.email) {
        accounts.push(account.email.clone());
        let manifest = serde_json::to_string(&accounts).map_err(auth_error)?;
        set_secret(app, SECRET_SCOPE, SECRET_KEY_ACCOUNTS, &manifest).await?;
    }

    Ok(())
}

pub async fn delete_account_keyring<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    email: &str,
) -> Result<(), Error> {
    let _ = delete_secret(app, SECRET_SCOPE, &format!("oauth/{email}")).await;
    let _ = delete_secret(app, "outlook-calendar/oauth", email).await;

    let mut accounts = load_account_emails(app).await?;
    accounts.retain(|e| e != email);
    let manifest = serde_json::to_string(&accounts).map_err(auth_error)?;
    set_secret(app, SECRET_SCOPE, SECRET_KEY_ACCOUNTS, &manifest).await?;

    Ok(())
}

async fn load_account_emails<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
) -> Result<Vec<String>, Error> {
    match get_secret(app, SECRET_SCOPE, SECRET_KEY_ACCOUNTS).await? {
        None => Ok(Vec::new()),
        Some(raw) => Ok(serde_json::from_str::<Vec<String>>(&raw).unwrap_or_default()),
    }
}

pub async fn read_client_keyring<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
) -> Result<Option<StoredOutlookClient>, Error> {
    match get_secret(app, SECRET_SCOPE, SECRET_KEY_CLIENT).await? {
        None => Ok(None),
        Some(raw) => serde_json::from_str(&raw)
            .map(Some)
            .map_err(|e| auth_error(format!("corrupt stored client: {e}"))),
    }
}

pub async fn write_client_keyring<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    client: &StoredOutlookClient,
) -> Result<(), Error> {
    let raw = serde_json::to_string(client).map_err(auth_error)?;
    set_secret(app, SECRET_SCOPE, SECRET_KEY_CLIENT, &raw).await
}

pub async fn delete_client_keyring<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
) -> Result<(), Error> {
    delete_secret(app, SECRET_SCOPE, SECRET_KEY_CLIENT).await
}

pub async fn load_client<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
) -> Result<Option<StoredOutlookClient>, Error> {
    let state = app.state::<OutlookAuthState>();
    let mut guard = state.client.lock().await;
    if let Some(cached) = guard.as_ref() {
        return Ok(cached.clone());
    }

    let loaded = match read_client_keyring(app).await? {
        Some(c) => Some(c),
        None => bundled_client(),
    };
    *guard = Some(loaded.clone());
    Ok(loaded)
}

pub async fn load_accounts<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
) -> Result<HashMap<String, StoredOutlookAccount>, Error> {
    let state = app.state::<OutlookAuthState>();
    let mut guard = state.accounts.lock().await;
    if let Some(cached) = guard.as_ref() {
        return Ok(cached.clone());
    }

    let emails = load_account_emails(app).await?;
    let mut accounts = HashMap::new();
    for email in emails {
        if let Some(account) = read_account_keyring(app, &email).await? {
            accounts.insert(email, account);
        }
    }

    *guard = Some(accounts.clone());
    Ok(accounts)
}

// === Connection IDs ===

/// Parse an Outlook connection ID string (`outlook:<email>`) into the account email.
pub fn parse_outlook_connection_id(connection_id: &str) -> Option<&str> {
    connection_id.strip_prefix("outlook:")
}

/// Returns connection IDs for all connected Outlook accounts (`outlook:<email>`).
pub async fn list_connection_ids<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
) -> Result<Vec<String>, Error> {
    let accounts = load_accounts(app).await?;
    Ok(accounts
        .keys()
        .map(|email| format!("outlook:{email}"))
        .collect())
}

pub async fn is_connected<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> bool {
    matches!(load_accounts(app).await, Ok(accounts) if !accounts.is_empty())
}

// === Token acquisition & per-account selection ===

/// Get a valid Outlook access token for a specific connection or default.
pub async fn access_token_for_connection<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    connection_id: &str,
) -> Result<String, Error> {
    let email_opt = if connection_id == "outlook-direct" {
        None
    } else if let Some(email) = parse_outlook_connection_id(connection_id) {
        Some(email)
    } else {
        return Err(auth_error(format!(
            "invalid outlook connection id '{connection_id}': expected format 'outlook:<email>' or 'outlook-direct'"
        )));
    };
    access_token_for_account(app, email_opt).await
}

/// Get a valid Outlook access token for an account, refreshing if expired.
pub async fn access_token_for_account<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    account_email: Option<&str>,
) -> Result<String, Error> {
    let accounts = load_accounts(app).await?;
    if accounts.is_empty() {
        return Err(auth_error("no outlook account connected"));
    }

    let account = match account_email {
        Some(email) => accounts
            .get(email)
            .ok_or_else(|| auth_error(format!("account not found: {email}")))?,
        None => accounts
            .values()
            .next()
            .ok_or_else(|| auth_error("no accounts available"))?,
    };

    let refresh_token = account
        .refresh_token
        .as_deref()
        .ok_or_else(|| auth_error(format!("no refresh token for account {}", account.email)))?
        .to_string();

    let state = app.state::<OutlookAuthState>();

    // 1. Fast path: check in-memory token cache
    {
        let tokens_guard = state.tokens.lock().await;
        if let Some(cached) = tokens_guard.get(&account.email)
            && cached.expires_at > Instant::now() + EXPIRY_MARGIN
        {
            return Ok(cached.access_token.clone());
        }
    }

    // 2. Per-account lock to prevent concurrent double-refresh
    let refresh_lock = {
        let mut locks = state.refresh_locks.lock().unwrap();
        locks.entry(account.email.clone()).or_default().clone()
    };
    let _refresh_guard = refresh_lock.lock().await;

    // 3. Re-check cache in case another task refreshed while waiting
    {
        let tokens_guard = state.tokens.lock().await;
        if let Some(cached) = tokens_guard.get(&account.email)
            && cached.expires_at > Instant::now() + EXPIRY_MARGIN
        {
            return Ok(cached.access_token.clone());
        }
    }

    // 4. Perform refresh without holding state.tokens
    let client_id = account.client_id.clone();
    let email = account.email.clone();
    let token = hypr_microsoft_oauth::refresh_access_token(
        &state.http,
        hypr_microsoft_oauth::DEFAULT_TOKEN_URI,
        &client_id,
        &refresh_token,
        None,
    )
    .await
    .map_err(|e| auth_error(format!("access-token refresh failed for {email}: {e}")))?;

    if let Some(new_refresh) = token.refresh_token.clone()
        && new_refresh != refresh_token
    {
        let mut updated = account.clone();
        updated.refresh_token = Some(new_refresh);
        write_account_keyring(app, &updated).await?;
        let mut accounts_guard = state.accounts.lock().await;
        if let Some(map) = &mut *accounts_guard {
            map.insert(updated.email.clone(), updated);
        }
    }

    let access_token = token.access_token.clone();
    {
        let mut tokens_guard = state.tokens.lock().await;
        tokens_guard.insert(
            account.email.clone(),
            CachedToken {
                access_token: token.access_token,
                expires_at: Instant::now() + Duration::from_secs(token.expires_in.unwrap_or(3600)),
            },
        );
    }

    Ok(access_token)
}

// === Status ===

pub async fn status<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
) -> Result<OutlookAccountStatus, Error> {
    let client = load_client(app).await?;
    let has_bundled = bundled_client().is_some();
    let accounts = load_accounts(app).await.unwrap_or_default();

    let connected = !accounts.is_empty();
    let account_list: Vec<OutlookConnectedAccount> = accounts
        .into_values()
        .map(|acc| OutlookConnectedAccount {
            email: acc.email,
            granted_scopes: acc.granted_scopes,
        })
        .collect();

    Ok(match client {
        None => OutlookAccountStatus {
            has_client: false,
            has_bundled_client: has_bundled,
            connected,
            client_id: None,
            accounts: account_list,
        },
        Some(client) => OutlookAccountStatus {
            has_client: true,
            has_bundled_client: has_bundled,
            connected,
            client_id: Some(client.client_id),
            accounts: account_list,
        },
    })
}

// === BYO Client configuration ===

pub async fn set_client_id<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    client_id: &str,
) -> Result<OutlookClientImportResult, Error> {
    let trimmed = client_id.trim();
    if trimmed.is_empty() {
        return Err(auth_error("client id cannot be empty"));
    }

    let client = StoredOutlookClient {
        client_id: trimmed.to_string(),
    };
    write_client_keyring(app, &client).await?;

    let state = app.state::<OutlookAuthState>();
    *state.client.lock().await = Some(Some(client.clone()));

    Ok(OutlookClientImportResult {
        client_id: client.client_id,
    })
}

// === Connect & Disconnect ===

pub async fn connect<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
) -> Result<OutlookAccountStatus, Error> {
    let client = load_client(app).await?.ok_or_else(|| {
        auth_error("no client configured; paste a client ID or use official build")
    })?;

    let state = app.state::<OutlookAuthState>();
    let (token, email) = hypr_microsoft_oauth::connect(
        &state.http,
        &client.client_id,
        DEFAULT_READ_SCOPES,
        CONNECT_TIMEOUT,
        hypr_microsoft_oauth::DEFAULT_AUTH_URI,
        hypr_microsoft_oauth::DEFAULT_TOKEN_URI,
        hypr_microsoft_oauth::DEFAULT_GRAPH_BASE_URL,
        |url| {
            app.opener()
                .open_url(url, None::<&str>)
                .map_err(|e| e.to_string())
        },
    )
    .await
    .map_err(|e| auth_error(e.to_string()))?;

    let granted_scopes = token
        .scope
        .as_deref()
        .map(|s| s.split_whitespace().map(ToString::to_string).collect())
        .unwrap_or_else(|| {
            DEFAULT_READ_SCOPES
                .iter()
                .map(|s| (*s).to_string())
                .collect()
        });

    let account = StoredOutlookAccount {
        email: email.clone(),
        client_id: client.client_id.clone(),
        refresh_token: token.refresh_token,
        granted_scopes,
    };

    write_account_keyring(app, &account).await?;

    // Update in-memory cache
    let mut accounts_guard = state.accounts.lock().await;
    if let Some(map) = &mut *accounts_guard {
        map.insert(email.clone(), account);
    } else {
        let mut map = HashMap::new();
        map.insert(email.clone(), account);
        *accounts_guard = Some(map);
    }

    // Cache access token
    let mut tokens_guard = state.tokens.lock().await;
    tokens_guard.insert(
        email,
        CachedToken {
            access_token: token.access_token,
            expires_at: Instant::now() + Duration::from_secs(token.expires_in.unwrap_or(3600)),
        },
    );

    status(app).await
}

pub async fn disconnect<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    email: Option<&str>,
) -> Result<OutlookAccountStatus, Error> {
    let state = app.state::<OutlookAuthState>();

    match email {
        Some(target) => {
            delete_account_keyring(app, target).await?;
            let mut accounts_guard = state.accounts.lock().await;
            if let Some(map) = &mut *accounts_guard {
                map.remove(target);
            }
            let mut tokens_guard = state.tokens.lock().await;
            tokens_guard.remove(target);
        }
        None => {
            let accounts = load_account_emails(app).await?;
            for acc in accounts {
                delete_account_keyring(app, &acc).await?;
            }
            *state.accounts.lock().await = Some(HashMap::new());
            state.tokens.lock().await.clear();
        }
    }

    status(app).await
}

pub async fn reset<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
) -> Result<OutlookAccountStatus, Error> {
    disconnect(app, None).await?;
    delete_client_keyring(app).await?;
    let state = app.state::<OutlookAuthState>();
    *state.client.lock().await = Some(bundled_client());
    status(app).await
}

// === Incremental Scope & Create Event ===

pub async fn ensure_write_scope<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    email: &str,
) -> Result<(), Error> {
    let accounts = load_accounts(app).await?;
    let account = accounts
        .get(email)
        .ok_or_else(|| auth_error(format!("account not found: {email}")))?;

    if account
        .granted_scopes
        .iter()
        .any(|s| s.eq_ignore_ascii_case(WRITE_EVENTS_SCOPE))
    {
        return Ok(());
    }

    let client = load_client(app)
        .await?
        .ok_or_else(|| auth_error("no client configured"))?;

    let state = app.state::<OutlookAuthState>();
    let token = hypr_microsoft_oauth::connect_incremental(
        &state.http,
        &client.client_id,
        DEFAULT_WRITE_SCOPES,
        CONNECT_TIMEOUT,
        hypr_microsoft_oauth::DEFAULT_AUTH_URI,
        hypr_microsoft_oauth::DEFAULT_TOKEN_URI,
        |url| {
            app.opener()
                .open_url(url, None::<&str>)
                .map_err(|e| e.to_string())
        },
    )
    .await
    .map_err(|e| auth_error(format!("failed to grant write scope: {e}")))?;

    let mut updated_scopes = account.granted_scopes.clone();
    if !updated_scopes
        .iter()
        .any(|s| s.eq_ignore_ascii_case(WRITE_EVENTS_SCOPE))
    {
        updated_scopes.push(WRITE_EVENTS_SCOPE.to_string());
    }

    let mut updated = account.clone();
    if let Some(new_refresh) = token.refresh_token.clone() {
        updated.refresh_token = Some(new_refresh);
    }
    updated.granted_scopes = updated_scopes;

    write_account_keyring(app, &updated).await?;

    let mut accounts_guard = state.accounts.lock().await;
    if let Some(map) = &mut *accounts_guard {
        map.insert(updated.email.clone(), updated);
    }

    let mut tokens_guard = state.tokens.lock().await;
    tokens_guard.insert(
        email.to_string(),
        CachedToken {
            access_token: token.access_token,
            expires_at: Instant::now() + Duration::from_secs(token.expires_in.unwrap_or(3600)),
        },
    );

    Ok(())
}

/// Validate that a calendar tracking ID is safe for URL usage and does not contain path traversal.
pub fn validate_calendar_id(id: &str) -> Result<(), Error> {
    let trimmed = id.trim();
    if trimmed.is_empty() {
        return Err(Error::Api("calendar id cannot be empty".to_string()));
    }
    if trimmed.contains('/')
        || trimmed.contains('\\')
        || trimmed.contains("..")
        || trimmed.chars().any(|c| c.is_control())
    {
        return Err(Error::Api(format!(
            "invalid calendar id '{id}': contains forbidden path traversal or control characters"
        )));
    }
    Ok(())
}

/// Resolve which Outlook account owns the target calendar based on connection_id.
pub fn resolve_target_account<'a>(
    accounts: &'a HashMap<String, StoredOutlookAccount>,
    connection_id: &str,
) -> Result<&'a StoredOutlookAccount, Error> {
    if connection_id == "outlook-direct" {
        accounts
            .values()
            .next()
            .ok_or_else(|| auth_error("no outlook account connected"))
    } else if let Some(email) = parse_outlook_connection_id(connection_id) {
        accounts
            .get(email)
            .ok_or_else(|| auth_error(format!("account not found: {email}")))
    } else {
        Err(auth_error(format!(
            "invalid outlook connection id '{connection_id}': expected format 'outlook:<email>' or 'outlook-direct'"
        )))
    }
}

pub async fn create_event<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    connection_id: &str,
    input: hypr_calendar_interface::CreateEventInput,
) -> Result<String, Error> {
    validate_calendar_id(&input.calendar_tracking_id)?;

    let accounts = load_accounts(app).await?;
    if accounts.is_empty() {
        return Err(hypr_calendar::Error::NotAuthenticated.into());
    }

    let target_account = resolve_target_account(&accounts, connection_id)?;
    let target_email = target_account.email.clone();

    ensure_write_scope(app, &target_email).await?;
    let token = access_token_for_account(app, Some(&target_email)).await?;

    let is_all_day = input.is_all_day.unwrap_or(false);
    let body = hypr_outlook_calendar::CreateEventBody {
        subject: input.title,
        body: input.notes.map(|notes| hypr_outlook_calendar::ItemBody {
            content_type: Some(hypr_outlook_calendar::BodyType::Text),
            content: Some(notes),
        }),
        location: input.location.map(|loc| hypr_outlook_calendar::Location {
            display_name: Some(loc),
            ..Default::default()
        }),
        start: hypr_outlook_calendar::DateTimeTimeZone {
            date_time: input.started_at,
            time_zone: Some("UTC".to_string()),
        },
        end: hypr_outlook_calendar::DateTimeTimeZone {
            date_time: input.ended_at,
            time_zone: Some("UTC".to_string()),
        },
        is_all_day: Some(is_all_day),
        ..Default::default()
    };

    let calendar_id = &input.calendar_tracking_id;
    let url = format!(
        "https://graph.microsoft.com/v1.0/me/calendars/{}/events",
        urlencoding::encode(calendar_id)
    );

    let state = app.state::<OutlookAuthState>();
    let response = state
        .http
        .post(&url)
        .header(reqwest::header::AUTHORIZATION, format!("Bearer {token}"))
        .json(&body)
        .send()
        .await
        .map_err(|e| Error::Api(e.to_string()))?;

    let status = response.status();
    let bytes = response
        .bytes()
        .await
        .map_err(|e| Error::Api(e.to_string()))?;

    if !status.is_success() {
        return Err(Error::Api(format!(
            "graph event creation failed with {status}: {}",
            String::from_utf8_lossy(&bytes)
        )));
    }

    let created: hypr_outlook_calendar::Event = serde_json::from_slice(&bytes)
        .map_err(|e| Error::Api(format!("failed to parse created event: {e}")))?;

    Ok(created.id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_outlook_connection_id() {
        assert_eq!(
            parse_outlook_connection_id("outlook:alice@contoso.com"),
            Some("alice@contoso.com")
        );
        assert_eq!(parse_outlook_connection_id("google:alice@gmail.com"), None);
        assert_eq!(parse_outlook_connection_id("ics"), None);
    }

    #[test]
    fn test_resolve_target_account_for_create_event() {
        let mut accounts = HashMap::new();
        accounts.insert(
            "alice@contoso.com".to_string(),
            StoredOutlookAccount {
                email: "alice@contoso.com".to_string(),
                client_id: "alice-client".to_string(),
                refresh_token: Some("alice-token".to_string()),
                granted_scopes: vec![],
            },
        );
        accounts.insert(
            "bob@contoso.com".to_string(),
            StoredOutlookAccount {
                email: "bob@contoso.com".to_string(),
                client_id: "bob-client".to_string(),
                refresh_token: Some("bob-token".to_string()),
                granted_scopes: vec![],
            },
        );

        // Resolves alice even if calendar_tracking_id is an opaque Graph ID
        let alice = resolve_target_account(&accounts, "outlook:alice@contoso.com").unwrap();
        assert_eq!(alice.email, "alice@contoso.com");

        // Resolves bob correctly
        let bob = resolve_target_account(&accounts, "outlook:bob@contoso.com").unwrap();
        assert_eq!(bob.email, "bob@contoso.com");

        // Non-connected account returns error
        let err = resolve_target_account(&accounts, "outlook:charlie@contoso.com");
        assert!(err.is_err());

        // Invalid format returns error
        assert!(resolve_target_account(&accounts, "google:alice@gmail.com").is_err());
        assert!(resolve_target_account(&accounts, "invalid").is_err());

        // outlook-direct falls back to an available account
        let direct = resolve_target_account(&accounts, "outlook-direct").unwrap();
        assert!(direct.email == "alice@contoso.com" || direct.email == "bob@contoso.com");
    }

    #[test]
    fn test_validate_calendar_id() {
        assert!(validate_calendar_id("AAMkAD12345").is_ok());
        assert!(validate_calendar_id("AQMkADAwATM3AAA=").is_ok());

        assert!(validate_calendar_id("").is_err());
        assert!(validate_calendar_id("   ").is_err());
        assert!(validate_calendar_id("../me/messages").is_err());
        assert!(validate_calendar_id("path/segment").is_err());
        assert!(validate_calendar_id("path\\segment").is_err());
        assert!(validate_calendar_id("id\nwith\rnewlines").is_err());
    }

    #[test]
    fn test_graph_calendar_response_to_struct_mapping() {
        let fixture = serde_json::json!({
            "@odata.context": "https://graph.microsoft.com/v1.0/$metadata#users('me')/calendars",
            "value": [
                {
                    "id": "AAMkAD12345",
                    "name": "Personal Calendar",
                    "color": "auto",
                    "hexColor": "#0078d4",
                    "isDefaultCalendar": true,
                    "canEdit": true,
                    "owner": {
                        "name": "Jane Doe",
                        "address": "jane@example.com"
                    }
                }
            ]
        });

        let resp: hypr_outlook_calendar::ListCalendarsResponse =
            serde_json::from_value(fixture).unwrap();
        assert_eq!(resp.value.len(), 1);

        let items = hypr_calendar::convert::convert_outlook_calendars(resp.value);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id, "AAMkAD12345");
        assert_eq!(items[0].title, "Personal Calendar");
        assert_eq!(
            items[0].provider,
            hypr_calendar_interface::CalendarProviderType::Outlook
        );
        assert_eq!(items[0].is_primary, Some(true));
        assert_eq!(items[0].can_edit, Some(true));
        assert_eq!(items[0].source.as_deref(), Some("Jane Doe"));
    }

    #[test]
    fn test_graph_event_response_to_struct_mapping() {
        let fixture = serde_json::json!({
            "@odata.context": "https://graph.microsoft.com/v1.0/$metadata#users('me')/calendarView",
            "value": [
                {
                    "id": "event-1",
                    "iCalUId": "ical-123@microsoft.com",
                    "subject": "Architecture Discussion",
                    "body": {
                        "contentType": "text",
                        "content": "Discuss Notare calendar architecture."
                    },
                    "start": {
                        "dateTime": "2026-11-03T10:00:00.0000000",
                        "timeZone": "UTC"
                    },
                    "end": {
                        "dateTime": "2026-11-03T11:00:00.0000000",
                        "timeZone": "UTC"
                    },
                    "location": {
                        "displayName": "Virtual Room A"
                    },
                    "onlineMeetingUrl": "https://teams.microsoft.com/l/meetup-join/123",
                    "isAllDay": false,
                    "isCancelled": false,
                    "showAs": "busy",
                    "isOrganizer": true,
                    "organizer": {
                        "emailAddress": {
                            "name": "Jane Doe",
                            "address": "jane@example.com"
                        }
                    },
                    "attendees": [
                        {
                            "type": "required",
                            "status": {
                                "response": "accepted"
                            },
                            "emailAddress": {
                                "name": "John Smith",
                                "address": "john@example.com"
                            }
                        }
                    ]
                }
            ]
        });

        let resp: hypr_outlook_calendar::ListEventsResponse =
            serde_json::from_value(fixture).unwrap();
        assert_eq!(resp.value.len(), 1);

        let events = hypr_calendar::convert::convert_outlook_events(resp.value, "calendar-cal-1");
        assert_eq!(events.len(), 1);
        let evt = &events[0];
        assert_eq!(evt.id, "event-1");
        assert_eq!(evt.calendar_id, "calendar-cal-1");
        assert_eq!(evt.title, "Architecture Discussion");
        assert_eq!(
            evt.meeting_link.as_deref(),
            Some("https://teams.microsoft.com/l/meetup-join/123")
        );
        assert_eq!(evt.location.as_deref(), Some("Virtual Room A"));
        assert_eq!(
            evt.description.as_deref(),
            Some("Discuss Notare calendar architecture.")
        );
        assert_eq!(evt.started_at, "2026-11-03T10:00:00.0000000");
        assert_eq!(evt.ended_at, "2026-11-03T11:00:00.0000000");
        assert_eq!(evt.timezone.as_deref(), Some("UTC"));
        assert!(!evt.is_all_day);
        assert_eq!(evt.status, hypr_calendar_interface::EventStatus::Confirmed);

        let org = evt.organizer.as_ref().unwrap();
        assert_eq!(org.name.as_deref(), Some("Jane Doe"));
        assert_eq!(org.email.as_deref(), Some("jane@example.com"));
        assert!(org.is_current_user);

        assert_eq!(evt.attendees.len(), 1);
        let att = &evt.attendees[0];
        assert_eq!(att.name.as_deref(), Some("John Smith"));
        assert_eq!(att.email.as_deref(), Some("john@example.com"));
        assert_eq!(
            att.status,
            hypr_calendar_interface::AttendeeStatus::Accepted
        );
    }
}

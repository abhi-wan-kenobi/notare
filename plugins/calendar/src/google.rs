//! Direct ("bring your own OAuth client") Google Calendar & Gmail integration.
//!
//! Features:
//! - Multi-account Google Calendar support (Issue #9): tokens persisted per account
//!   at keyring key `google-calendar/oauth/<account_email>`. Legacy single `oauth`
//!   key is migrated on first run using `userinfo.email`.
//! - Incremental write scope (Issue #17): `https://www.googleapis.com/auth/calendar.events`
//!   is requested incrementally with `include_granted_scopes=true` and `prompt=consent`
//!   when the user first creates an event.
//! - Opt-in Gmail client: compile-time `NOTARE_GMAIL_CLIENT_ID/_SECRET` for creating
//!   email drafts (`users.drafts.create`).

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use hypr_google_oauth::{ClientCredentials, ClientJsonKind};
use tauri::Manager;
use tauri_plugin_opener::OpenerExt;

use crate::error::Error;

/// Secure-store (OS keyring) location for the credentials blob.
pub const SECRET_SCOPE: &str = "google-calendar";
pub const SECRET_KEY_LEGACY: &str = "oauth";
pub const SECRET_KEY_ACCOUNTS: &str = "accounts";
pub const SECRET_KEY_CLIENT: &str = "client";

pub const GMAIL_SECRET_SCOPE: &str = "google-mail";

/// Default read-only scopes requested on initial connection.
pub const DEFAULT_READ_SCOPES: &[&str] = &[
    "https://www.googleapis.com/auth/calendar.readonly",
    "https://www.googleapis.com/auth/calendar.events.readonly",
    "https://www.googleapis.com/auth/userinfo.email",
];

/// Incremental write scope required for event creation.
pub const WRITE_EVENTS_SCOPE: &str = "https://www.googleapis.com/auth/calendar.events";

/// Gmail scopes for compose/draft creation.
pub const GMAIL_SCOPES: &[&str] = &[
    "https://www.googleapis.com/auth/gmail.compose",
    "https://www.googleapis.com/auth/userinfo.email",
];

/// How long we wait for the user to finish the browser consent flow.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(300);
/// Refresh the access token this long before it actually expires.
const EXPIRY_MARGIN: Duration = Duration::from_secs(60);

/// Legacy single-account credential shape (used for deserializing old keyring entries).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StoredGoogleCredentials {
    pub client_id: String,
    pub client_secret: String,
    pub auth_uri: String,
    pub token_uri: String,
    #[serde(default)]
    pub client_kind: Option<String>,
    #[serde(default)]
    pub refresh_token: Option<String>,
}

impl StoredGoogleCredentials {
    pub fn client(&self) -> ClientCredentials {
        ClientCredentials {
            client_id: self.client_id.clone(),
            client_secret: self.client_secret.clone(),
            auth_uri: self.auth_uri.clone(),
            token_uri: self.token_uri.clone(),
        }
    }
}

/// What gets serialized into each per-account keyring entry (`google-calendar/oauth/<email>`).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct StoredGoogleAccount {
    pub email: String,
    pub client_id: String,
    pub client_secret: String,
    pub auth_uri: String,
    pub token_uri: String,
    #[serde(default)]
    pub client_kind: Option<String>,
    #[serde(default)]
    pub refresh_token: Option<String>,
    #[serde(default)]
    pub granted_scopes: Vec<String>,
}

impl StoredGoogleAccount {
    pub fn client(&self) -> ClientCredentials {
        ClientCredentials {
            client_id: self.client_id.clone(),
            client_secret: self.client_secret.clone(),
            auth_uri: self.auth_uri.clone(),
            token_uri: self.token_uri.clone(),
        }
    }
}

/// Imported or bundled client configuration.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct StoredGoogleClient {
    pub client_id: String,
    pub client_secret: String,
    pub auth_uri: String,
    pub token_uri: String,
    #[serde(default)]
    pub client_kind: Option<String>,
}

impl StoredGoogleClient {
    pub fn client(&self) -> ClientCredentials {
        ClientCredentials {
            client_id: self.client_id.clone(),
            client_secret: self.client_secret.clone(),
            auth_uri: self.auth_uri.clone(),
            token_uri: self.token_uri.clone(),
        }
    }
}

/// Per-account representation returned to the frontend.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct GoogleConnectedAccount {
    pub email: String,
    pub granted_scopes: Vec<String>,
}

/// Status surfaced to the frontend for Google Calendar.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct GoogleAccountStatus {
    /// A client json has been imported (BYO client) or bundled client is available.
    pub has_client: bool,
    /// A bundled first-party client is compiled into this build.
    pub has_bundled_client: bool,
    /// At least one Google account is connected.
    pub connected: bool,
    /// `"installed"` or `"web"` (when a client is present).
    pub client_kind: Option<String>,
    /// The client id, so the user can tell which GCP client is in use.
    pub client_id: Option<String>,
    /// All currently connected Google accounts.
    #[serde(default)]
    pub accounts: Vec<GoogleConnectedAccount>,
}

/// Status surfaced to the frontend for Gmail.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct GmailAccountStatus {
    /// A Gmail client is configured at compile time.
    pub has_client: bool,
    /// At least one Gmail account is connected.
    pub connected: bool,
    /// Connected Gmail account emails.
    #[serde(default)]
    pub accounts: Vec<String>,
}

/// Result of importing a client json.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct GoogleClientImportResult {
    pub client_id: String,
    pub client_kind: String,
    pub warning_web_client: bool,
}

#[derive(Clone)]
struct CachedToken {
    access_token: String,
    expires_at: Instant,
}

/// Managed state: in-memory credential + access-token cache so the OS keyring
/// is only touched on first use and on mutations.
pub struct GoogleAuthState {
    pub http: reqwest::Client,
    /// Active client config (BYO or bundled).
    client: tokio::sync::Mutex<Option<Option<StoredGoogleClient>>>,
    /// Cached map of account email -> StoredGoogleAccount.
    accounts: tokio::sync::Mutex<Option<HashMap<String, StoredGoogleAccount>>>,
    /// In-memory access token cache keyed by account email.
    tokens: tokio::sync::Mutex<HashMap<String, CachedToken>>,
    /// Per-account refresh locks to prevent double-refresh race.
    refresh_locks: std::sync::Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>,
    /// Cached map of Gmail accounts.
    gmail_accounts: tokio::sync::Mutex<Option<HashMap<String, StoredGoogleAccount>>>,
    /// In-memory Gmail access token cache keyed by account email.
    gmail_tokens: tokio::sync::Mutex<HashMap<String, CachedToken>>,
    /// Per-account Gmail refresh locks to prevent double-refresh race.
    gmail_refresh_locks: std::sync::Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>,
}

impl Default for GoogleAuthState {
    fn default() -> Self {
        Self {
            http: reqwest::Client::new(),
            client: tokio::sync::Mutex::new(None),
            accounts: tokio::sync::Mutex::new(None),
            tokens: tokio::sync::Mutex::new(HashMap::new()),
            refresh_locks: std::sync::Mutex::new(HashMap::new()),
            gmail_accounts: tokio::sync::Mutex::new(None),
            gmail_tokens: tokio::sync::Mutex::new(HashMap::new()),
            gmail_refresh_locks: std::sync::Mutex::new(HashMap::new()),
        }
    }
}

pub fn auth_error(message: impl std::fmt::Display) -> Error {
    Error::Auth(format!("google: {message}"))
}

/// The bundled first-party OAuth client for Calendar.
pub fn bundled_client() -> Option<StoredGoogleClient> {
    let client_id = option_env!("NOTARE_GOOGLE_CLIENT_ID").filter(|s| !s.is_empty())?;
    let client_secret = option_env!("NOTARE_GOOGLE_CLIENT_SECRET").filter(|s| !s.is_empty())?;
    Some(StoredGoogleClient {
        client_id: client_id.to_string(),
        client_secret: client_secret.to_string(),
        auth_uri: "https://accounts.google.com/o/oauth2/auth".to_string(),
        token_uri: "https://oauth2.googleapis.com/token".to_string(),
        client_kind: Some("installed".to_string()),
    })
}

/// The bundled first-party OAuth client for Gmail.
pub fn gmail_bundled_client() -> Option<StoredGoogleClient> {
    let client_id = option_env!("NOTARE_GMAIL_CLIENT_ID").filter(|s| !s.is_empty())?;
    let client_secret = option_env!("NOTARE_GMAIL_CLIENT_SECRET").filter(|s| !s.is_empty())?;
    Some(StoredGoogleClient {
        client_id: client_id.to_string(),
        client_secret: client_secret.to_string(),
        auth_uri: "https://accounts.google.com/o/oauth2/auth".to_string(),
        token_uri: "https://oauth2.googleapis.com/token".to_string(),
        client_kind: Some("installed".to_string()),
    })
}

// === Keyring access helpers ===

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

// === Account Migration ===

/// Pure helper for migrating legacy single-credential blob into a StoredGoogleAccount.
pub fn migrate_legacy_blob(legacy_raw: &str, email: &str) -> Result<StoredGoogleAccount, Error> {
    let creds: StoredGoogleCredentials = serde_json::from_str(legacy_raw)
        .map_err(|e| auth_error(format!("invalid legacy blob: {e}")))?;
    Ok(StoredGoogleAccount {
        email: email.to_string(),
        client_id: creds.client_id,
        client_secret: creds.client_secret,
        auth_uri: creds.auth_uri,
        token_uri: creds.token_uri,
        client_kind: creds.client_kind,
        refresh_token: creds.refresh_token,
        granted_scopes: DEFAULT_READ_SCOPES.iter().map(|s| s.to_string()).collect(),
    })
}

/// Fetch the user's email address from Google's userinfo endpoint using an access token.
pub async fn fetch_user_email(http: &reqwest::Client, access_token: &str) -> Result<String, Error> {
    let response = http
        .get("https://www.googleapis.com/oauth2/v2/userinfo")
        .header(
            reqwest::header::AUTHORIZATION,
            format!("Bearer {access_token}"),
        )
        .send()
        .await
        .map_err(auth_error)?;

    if !response.status().is_success() {
        return Err(auth_error(format!(
            "userinfo endpoint returned status {}",
            response.status()
        )));
    }

    #[derive(serde::Deserialize)]
    struct UserInfo {
        email: Option<String>,
    }

    let user_info: UserInfo = response
        .json()
        .await
        .map_err(|e| auth_error(format!("failed to parse userinfo: {e}")))?;

    user_info
        .email
        .ok_or_else(|| auth_error("userinfo response did not contain an email"))
}

/// Perform one-time migration of legacy single `oauth` key into `<email>` using `userinfo.email`.
pub async fn migrate_legacy_account_if_needed<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
) -> Result<Option<String>, Error> {
    let legacy_raw = match get_secret(app, SECRET_SCOPE, SECRET_KEY_LEGACY).await? {
        Some(raw) => raw,
        None => return Ok(None),
    };

    let legacy_creds: StoredGoogleCredentials = match serde_json::from_str(&legacy_raw) {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("corrupt legacy google credentials: {e}");
            return Ok(None);
        }
    };

    let refresh_token = match legacy_creds.refresh_token.as_ref() {
        Some(rt) if !rt.is_empty() => rt,
        _ => {
            // No refresh token; remove stale legacy key.
            let _ = delete_secret(app, SECRET_SCOPE, SECRET_KEY_LEGACY).await;
            return Ok(None);
        }
    };

    let state = app.state::<GoogleAuthState>();
    let token_resp = match hypr_google_oauth::refresh_access_token(
        &state.http,
        &legacy_creds.client(),
        refresh_token,
    )
    .await
    {
        Ok(t) => t,
        Err(e) => {
            tracing::warn!("failed to refresh access token during migration: {e}");
            return Ok(None);
        }
    };

    let email = match fetch_user_email(&state.http, &token_resp.access_token).await {
        Ok(e) => e,
        Err(e) => {
            tracing::warn!("failed to fetch userinfo.email during migration: {e}");
            return Ok(None);
        }
    };

    let migrated = migrate_legacy_blob(&legacy_raw, &email)?;
    write_account_keyring(app, &migrated).await?;
    let _ = delete_secret(app, SECRET_SCOPE, SECRET_KEY_LEGACY).await;
    tracing::info!("migrated legacy google calendar account into {email}");
    Ok(Some(email))
}

// === Per-account keyring I/O ===

pub async fn read_account_keyring<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    email: &str,
) -> Result<Option<StoredGoogleAccount>, Error> {
    // Check primary keyring key: scope "google-calendar", key "oauth/<email>"
    let raw = match get_secret(app, SECRET_SCOPE, &format!("oauth/{email}")).await? {
        Some(raw) => Some(raw),
        // Fallback check: scope "google-calendar/oauth", key "<email>"
        None => get_secret(app, "google-calendar/oauth", email).await?,
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
    account: &StoredGoogleAccount,
) -> Result<(), Error> {
    let raw = serde_json::to_string(account).map_err(auth_error)?;
    // Write primary key: google-calendar:oauth/<email>
    set_secret(app, SECRET_SCOPE, &format!("oauth/{}", account.email), &raw).await?;
    // Write compatibility key: google-calendar/oauth:<email>
    let _ = set_secret(app, "google-calendar/oauth", &account.email, &raw).await;

    // Update the accounts manifest
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
    let _ = delete_secret(app, "google-calendar/oauth", email).await;

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

// === Client configuration I/O ===

pub async fn read_client_keyring<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
) -> Result<Option<StoredGoogleClient>, Error> {
    match get_secret(app, SECRET_SCOPE, SECRET_KEY_CLIENT).await? {
        None => Ok(None),
        Some(raw) => serde_json::from_str(&raw)
            .map(Some)
            .map_err(|e| auth_error(format!("corrupt stored client: {e}"))),
    }
}

pub async fn write_client_keyring<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    client: &StoredGoogleClient,
) -> Result<(), Error> {
    let raw = serde_json::to_string(client).map_err(auth_error)?;
    set_secret(app, SECRET_SCOPE, SECRET_KEY_CLIENT, &raw).await
}

pub async fn delete_client_keyring<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
) -> Result<(), Error> {
    delete_secret(app, SECRET_SCOPE, SECRET_KEY_CLIENT).await
}

// === Accounts and client state loading ===

pub async fn load_client<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
) -> Result<Option<StoredGoogleClient>, Error> {
    let state = app.state::<GoogleAuthState>();
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
) -> Result<HashMap<String, StoredGoogleAccount>, Error> {
    let state = app.state::<GoogleAuthState>();
    let mut guard = state.accounts.lock().await;
    if let Some(cached) = guard.as_ref() {
        return Ok(cached.clone());
    }

    // Check for legacy migration on first load
    let _ = migrate_legacy_account_if_needed(app).await;

    let emails = load_account_emails(app).await?;
    let mut map = HashMap::new();
    for email in emails {
        if let Some(account) = read_account_keyring(app, &email).await? {
            map.insert(email, account);
        }
    }

    *guard = Some(map.clone());
    Ok(map)
}

// === Connection ID parsing ===

/// Extracts the account email from a connection ID.
/// - `"google:alice@example.com"` -> `Some("alice@example.com")`
/// - `"google-direct"` -> `None` (indicates default/first account)
/// - other strings -> `Some(connection_id)`
pub fn parse_google_connection_id(connection_id: &str) -> Option<&str> {
    if let Some(email) = connection_id.strip_prefix("google:") {
        Some(email)
    } else if connection_id == "google-direct" {
        None
    } else {
        Some(connection_id)
    }
}

/// Returns connection IDs for all connected Google accounts (`google:<email>`).
pub async fn list_connection_ids<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
) -> Result<Vec<String>, Error> {
    let accounts = load_accounts(app).await?;
    Ok(accounts
        .keys()
        .map(|email| format!("google:{email}"))
        .collect())
}

pub async fn is_connected<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> bool {
    matches!(load_accounts(app).await, Ok(accounts) if !accounts.is_empty())
}

// === Token acquisition & per-account selection ===

/// Get a valid Google access token for a specific connection or default.
pub async fn access_token_for_connection<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    connection_id: &str,
) -> Result<String, Error> {
    let email_opt = parse_google_connection_id(connection_id);
    access_token_for_account(app, email_opt).await
}

/// Get a valid Google access token for an account, refreshing if expired.
pub async fn access_token_for_account<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    account_email: Option<&str>,
) -> Result<String, Error> {
    let accounts = load_accounts(app).await?;
    let account = match account_email {
        Some(email) => accounts
            .get(email)
            .ok_or_else(|| auth_error(format!("Google account {email} is not connected")))?,
        None => accounts
            .values()
            .next()
            .ok_or_else(|| hypr_calendar::Error::NotAuthenticated)?,
    };

    let refresh_token = account
        .refresh_token
        .clone()
        .ok_or_else(|| hypr_calendar::Error::NotAuthenticated)?;

    let state = app.state::<GoogleAuthState>();

    // 1. Fast path: check in-memory token cache
    {
        let tokens_guard = state.tokens.lock().await;
        if let Some(cached) = tokens_guard.get(&account.email)
            && cached.expires_at.saturating_duration_since(Instant::now()) > EXPIRY_MARGIN
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

    // 3. Re-check cache in case another task refreshed while we waited
    {
        let tokens_guard = state.tokens.lock().await;
        if let Some(cached) = tokens_guard.get(&account.email)
            && cached.expires_at.saturating_duration_since(Instant::now()) > EXPIRY_MARGIN
        {
            return Ok(cached.access_token.clone());
        }
    }

    // 4. Refresh token without holding state.tokens
    let client = account.client();
    let email = account.email.clone();
    let token = hypr_google_oauth::refresh_access_token(&state.http, &client, &refresh_token)
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
/// Fallback for single-account callers: returns access token for the first connected account.
#[allow(dead_code)]
pub async fn access_token<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> Result<String, Error> {
    access_token_for_account(app, None).await
}

// === Status ===

pub async fn status<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
) -> Result<GoogleAccountStatus, Error> {
    let client = load_client(app).await?;
    let has_bundled = bundled_client().is_some();
    let accounts = load_accounts(app).await.unwrap_or_default();

    let connected = !accounts.is_empty();
    let account_list: Vec<GoogleConnectedAccount> = accounts
        .into_values()
        .map(|acc| GoogleConnectedAccount {
            email: acc.email,
            granted_scopes: acc.granted_scopes,
        })
        .collect();

    Ok(match client {
        None => GoogleAccountStatus {
            has_client: false,
            has_bundled_client: has_bundled,
            connected,
            client_kind: None,
            client_id: None,
            accounts: account_list,
        },
        Some(client) => GoogleAccountStatus {
            has_client: true,
            has_bundled_client: has_bundled,
            connected,
            client_kind: client.client_kind,
            client_id: Some(client.client_id),
            accounts: account_list,
        },
    })
}

// === Incremental Scope & Auth URL construction ===

/// Build the consent-screen URL for incremental authorization.
/// Requests `include_granted_scopes=true` and `prompt=consent` for the new scope.
pub fn build_incremental_auth_url(
    creds: &ClientCredentials,
    redirect_uri: &str,
    new_scope: &str,
    state: &str,
    code_challenge: &str,
) -> String {
    let params = [
        ("client_id", creds.client_id.as_str()),
        ("redirect_uri", redirect_uri),
        ("response_type", "code"),
        ("scope", new_scope),
        ("access_type", "offline"),
        ("include_granted_scopes", "true"),
        ("prompt", "consent"),
        ("state", state),
        ("code_challenge", code_challenge),
        ("code_challenge_method", "S256"),
    ];
    let query = params
        .iter()
        .map(|(k, v)| format!("{}={}", k, urlencoding::encode(v)))
        .collect::<Vec<_>>()
        .join("&");
    format!("{}?{}", creds.auth_uri, query)
}

/// Run loopback authorization with incremental scope.
pub async fn connect_incremental(
    http: &reqwest::Client,
    creds: &ClientCredentials,
    new_scope: &str,
    timeout: Duration,
    open_browser: impl FnOnce(&str) -> Result<(), String>,
) -> Result<hypr_google_oauth::TokenResponse, Error> {
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .map_err(|e| auth_error(e.to_string()))?;
    let port = listener
        .local_addr()
        .map_err(|e| auth_error(e.to_string()))?
        .port();
    let redirect_uri = format!("http://127.0.0.1:{port}/");

    let state = uuid::Uuid::new_v4().simple().to_string();
    let pkce = hypr_google_oauth::generate_pkce();
    let auth_url =
        build_incremental_auth_url(creds, &redirect_uri, new_scope, &state, &pkce.challenge);

    open_browser(&auth_url).map_err(|e| auth_error(format!("failed to open browser: {e}")))?;

    let code = tokio::time::timeout(
        timeout,
        hypr_oauth_loopback::handle_connections(listener, &state),
    )
    .await
    .map_err(|_| auth_error("redirect timeout"))?
    .map_err(|e| auth_error(e.to_string()))?;

    let token = hypr_google_oauth::exchange_code(http, creds, &code, &redirect_uri, &pkce.verifier)
        .await
        .map_err(auth_error)?;

    Ok(token)
}

// === Connect Flow ===

/// Run the browser consent flow and persist the account refresh token.
pub async fn connect<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
) -> Result<GoogleAccountStatus, Error> {
    let client = load_client(app).await?.ok_or_else(|| {
        auth_error("no Google OAuth client available — add your own under Advanced settings")
    })?;

    let state = app.state::<GoogleAuthState>();
    let http = state.http.clone();
    let opener_app = app.clone();

    let token = hypr_google_oauth::connect(
        &http,
        &client.client(),
        DEFAULT_READ_SCOPES,
        CONNECT_TIMEOUT,
        move |url| {
            opener_app
                .opener()
                .open_url(url, None::<&str>)
                .map_err(|e| e.to_string())
        },
    )
    .await
    .map_err(auth_error)?;

    let email = fetch_user_email(&http, &token.access_token).await?;

    let account = StoredGoogleAccount {
        email: email.clone(),
        client_id: client.client_id,
        client_secret: client.client_secret,
        auth_uri: client.auth_uri,
        token_uri: client.token_uri,
        client_kind: client.client_kind,
        refresh_token: token.refresh_token,
        granted_scopes: DEFAULT_READ_SCOPES.iter().map(|s| s.to_string()).collect(),
    };

    write_account_keyring(app, &account).await?;

    let mut accounts_guard = state.accounts.lock().await;
    if let Some(map) = &mut *accounts_guard {
        map.insert(email.clone(), account);
    } else {
        let mut map = HashMap::new();
        map.insert(email.clone(), account);
        *accounts_guard = Some(map);
    }

    if let Some(expires_in) = token.expires_in {
        let mut tokens_guard = state.tokens.lock().await;
        tokens_guard.insert(
            email,
            CachedToken {
                access_token: token.access_token,
                expires_at: Instant::now() + Duration::from_secs(expires_in),
            },
        );
    }

    status(app).await
}

/// Disconnect a single account by email, or all accounts if `email` is None.
pub async fn disconnect<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    email: Option<&str>,
) -> Result<GoogleAccountStatus, Error> {
    let state = app.state::<GoogleAuthState>();
    let accounts = load_accounts(app).await?;

    match email {
        Some(target) => {
            if let Some(account) = accounts.get(target) {
                if let Some(rt) = &account.refresh_token {
                    let _ = hypr_google_oauth::revoke_token(
                        &state.http,
                        hypr_google_oauth::DEFAULT_REVOKE_URI,
                        rt,
                    )
                    .await;
                }
                delete_account_keyring(app, target).await?;
                let mut accounts_guard = state.accounts.lock().await;
                if let Some(map) = &mut *accounts_guard {
                    map.remove(target);
                }
                let mut tokens_guard = state.tokens.lock().await;
                tokens_guard.remove(target);
            }
        }
        None => {
            for (email, account) in accounts {
                if let Some(rt) = &account.refresh_token {
                    let _ = hypr_google_oauth::revoke_token(
                        &state.http,
                        hypr_google_oauth::DEFAULT_REVOKE_URI,
                        rt,
                    )
                    .await;
                }
                delete_account_keyring(app, &email).await?;
            }
            *state.accounts.lock().await = Some(HashMap::new());
            state.tokens.lock().await.clear();
        }
    }

    status(app).await
}

/// Reset everything: disconnect all accounts AND delete imported client JSON.
pub async fn reset<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
) -> Result<GoogleAccountStatus, Error> {
    let _ = disconnect(app, None).await;
    delete_client_keyring(app).await?;
    let state = app.state::<GoogleAuthState>();
    *state.client.lock().await = Some(None);
    status(app).await
}

/// Import a client JSON string.
pub async fn import_client_json<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    json: &str,
) -> Result<GoogleClientImportResult, Error> {
    let (client, kind) = hypr_google_oauth::parse_client_json(json).map_err(auth_error)?;
    let kind_str = match kind {
        ClientJsonKind::Installed => "installed",
        ClientJsonKind::Web => "web",
    };

    let stored = StoredGoogleClient {
        client_id: client.client_id.clone(),
        client_secret: client.client_secret,
        auth_uri: client.auth_uri,
        token_uri: client.token_uri,
        client_kind: Some(kind_str.to_string()),
    };

    write_client_keyring(app, &stored).await?;
    let state = app.state::<GoogleAuthState>();
    *state.client.lock().await = Some(Some(stored));

    Ok(GoogleClientImportResult {
        client_id: client.client_id,
        client_kind: kind_str.to_string(),
        warning_web_client: kind == ClientJsonKind::Web,
    })
}

/// Import a client JSON from a file.
pub async fn import_client_file<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    path: &str,
) -> Result<GoogleClientImportResult, Error> {
    let json = tokio::fs::read_to_string(path)
        .await
        .map_err(|e| auth_error(format!("could not read {path}: {e}")))?;
    import_client_json(app, &json).await
}

// === Ensure Write Scope & Create Event ===

/// Ensure that the account has the write scope granted.
/// If not, runs incremental authorization with `include_granted_scopes=true`.
pub async fn ensure_write_scope<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    email: &str,
) -> Result<(), Error> {
    let accounts = load_accounts(app).await?;
    let account = accounts
        .get(email)
        .ok_or_else(|| auth_error(format!("account {email} not found")))?;

    if account
        .granted_scopes
        .iter()
        .any(|s| s == WRITE_EVENTS_SCOPE)
    {
        return Ok(());
    }

    let state = app.state::<GoogleAuthState>();
    let http = state.http.clone();
    let opener_app = app.clone();
    let creds = account.client();

    let token = connect_incremental(
        &http,
        &creds,
        WRITE_EVENTS_SCOPE,
        CONNECT_TIMEOUT,
        move |url| {
            opener_app
                .opener()
                .open_url(url, None::<&str>)
                .map_err(|e| e.to_string())
        },
    )
    .await?;

    if let Some(scope_str) = &token.scope
        && !scope_str
            .split_whitespace()
            .any(|s| s == WRITE_EVENTS_SCOPE)
    {
        return Err(auth_error(format!(
            "write scope was not granted in incremental authorization: granted scopes are '{scope_str}'"
        )));
    }

    let mut updated = account.clone();
    if let Some(new_rt) = token.refresh_token {
        updated.refresh_token = Some(new_rt);
    }
    if !updated
        .granted_scopes
        .iter()
        .any(|s| s == WRITE_EVENTS_SCOPE)
    {
        updated.granted_scopes.push(WRITE_EVENTS_SCOPE.to_string());
    }

    write_account_keyring(app, &updated).await?;
    let mut accounts_guard = state.accounts.lock().await;
    if let Some(map) = &mut *accounts_guard {
        map.insert(updated.email.clone(), updated);
    }

    Ok(())
}

/// Helper HTTP client implementation for Google REST calls.
pub struct GoogleRestHttp {
    client: reqwest::Client,
    base_url: String,
}

impl GoogleRestHttp {
    pub fn new(base_url: &str, access_token: &str) -> Result<Self, Error> {
        let auth_value = format!("Bearer {access_token}")
            .parse()
            .map_err(|e| auth_error(format!("invalid auth header: {e}")))?;
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(reqwest::header::AUTHORIZATION, auth_value);
        let client = reqwest::Client::builder()
            .default_headers(headers)
            .build()
            .map_err(|e| auth_error(format!("failed to build client: {e}")))?;
        Ok(Self {
            client,
            base_url: base_url.trim_end_matches('/').to_string(),
        })
    }

    async fn request(
        &self,
        method: reqwest::Method,
        path: &str,
        body: Option<(Vec<u8>, String)>,
    ) -> Result<Vec<u8>, hypr_http::Error> {
        let url = if path.starts_with("http://") || path.starts_with("https://") {
            path.to_string()
        } else {
            format!("{}{}", self.base_url, path)
        };
        let mut request = self.client.request(method, &url);
        if let Some((body, content_type)) = body {
            request = request
                .header(reqwest::header::CONTENT_TYPE, content_type)
                .body(body);
        }
        let response = request.send().await?;
        let status = response.status();
        let bytes = response.bytes().await?;
        if !status.is_success() {
            return Err(format!(
                "google api returned {status}: {}",
                String::from_utf8_lossy(&bytes)
            )
            .into());
        }
        Ok(bytes.to_vec())
    }
}

impl hypr_http::HttpClient for GoogleRestHttp {
    async fn get(&self, path: &str) -> Result<Vec<u8>, hypr_http::Error> {
        self.request(reqwest::Method::GET, path, None).await
    }

    async fn post(
        &self,
        path: &str,
        body: Vec<u8>,
        content_type: &str,
    ) -> Result<Vec<u8>, hypr_http::Error> {
        self.request(
            reqwest::Method::POST,
            path,
            Some((body, content_type.to_string())),
        )
        .await
    }

    async fn put(&self, path: &str, body: Vec<u8>) -> Result<Vec<u8>, hypr_http::Error> {
        self.request(
            reqwest::Method::PUT,
            path,
            Some((body, "application/json".to_string())),
        )
        .await
    }

    async fn patch(&self, path: &str, body: Vec<u8>) -> Result<Vec<u8>, hypr_http::Error> {
        self.request(
            reqwest::Method::PATCH,
            path,
            Some((body, "application/json".to_string())),
        )
        .await
    }

    async fn delete(&self, path: &str) -> Result<Vec<u8>, hypr_http::Error> {
        self.request(reqwest::Method::DELETE, path, None).await
    }
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

/// Resolve which Google account owns the target calendar based on connection_id.
pub fn resolve_target_account<'a>(
    accounts: &'a HashMap<String, StoredGoogleAccount>,
    connection_id: &str,
) -> Result<&'a StoredGoogleAccount, Error> {
    match parse_google_connection_id(connection_id) {
        Some(email) => accounts
            .get(email)
            .ok_or_else(|| auth_error(format!("Google account {email} is not connected"))),
        None => accounts
            .values()
            .next()
            .ok_or_else(|| hypr_calendar::Error::NotAuthenticated.into()),
    }
}

/// Parse an ISO or RFC 3339 string into an EventDateTime without discarding parse errors.
pub fn parse_event_datetime(
    is_all_day: bool,
    s: &str,
) -> Result<hypr_google_calendar::EventDateTime, Error> {
    if is_all_day {
        let date = chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d")
            .or_else(|_| chrono::DateTime::parse_from_rfc3339(s).map(|dt| dt.date_naive()))
            .map_err(|e| Error::Api(format!("failed to parse all-day event date '{s}': {e}")))?;
        Ok(hypr_google_calendar::EventDateTime {
            date: Some(date),
            date_time: None,
            time_zone: None,
        })
    } else {
        let date_time = chrono::DateTime::parse_from_rfc3339(s)
            .or_else(|_| {
                chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S")
                    .map(|ndt| ndt.and_utc().fixed_offset())
            })
            .map_err(|e| Error::Api(format!("failed to parse event datetime '{s}': {e}")))?;
        Ok(hypr_google_calendar::EventDateTime {
            date: None,
            date_time: Some(date_time),
            time_zone: None,
        })
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
    let start = parse_event_datetime(is_all_day, &input.started_at)?;
    let end = parse_event_datetime(is_all_day, &input.ended_at)?;

    let req = hypr_google_calendar::CreateEventRequest {
        calendar_id: input.calendar_tracking_id,
        event: hypr_google_calendar::CreateEventBody {
            summary: input.title,
            description: input.notes,
            location: input.location,
            start,
            end,
            ..Default::default()
        },
    };

    let http = GoogleRestHttp::new("https://www.googleapis.com", &token)?;
    let client = hypr_google_calendar::GoogleCalendarClient::new(http);
    let created = client
        .create_event(req)
        .await
        .map_err(|e| Error::Api(e.to_string()))?;

    Ok(created.id)
}

// === Gmail Opt-In Client ===

async fn load_gmail_account_emails<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
) -> Result<Vec<String>, Error> {
    match get_secret(app, GMAIL_SECRET_SCOPE, SECRET_KEY_ACCOUNTS).await? {
        None => Ok(Vec::new()),
        Some(raw) => Ok(serde_json::from_str::<Vec<String>>(&raw).unwrap_or_default()),
    }
}

pub async fn read_gmail_account_keyring<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    email: &str,
) -> Result<Option<StoredGoogleAccount>, Error> {
    let raw = match get_secret(app, GMAIL_SECRET_SCOPE, &format!("oauth/{email}")).await? {
        Some(raw) => Some(raw),
        None => get_secret(app, "google-mail/oauth", email).await?,
    };

    match raw {
        None => Ok(None),
        Some(raw) => serde_json::from_str(&raw)
            .map(Some)
            .map_err(|e| auth_error(format!("corrupt gmail credentials for {email}: {e}"))),
    }
}

pub async fn write_gmail_account_keyring<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    account: &StoredGoogleAccount,
) -> Result<(), Error> {
    let raw = serde_json::to_string(account).map_err(auth_error)?;
    set_secret(
        app,
        GMAIL_SECRET_SCOPE,
        &format!("oauth/{}", account.email),
        &raw,
    )
    .await?;
    let _ = set_secret(app, "google-mail/oauth", &account.email, &raw).await;

    let mut accounts = load_gmail_account_emails(app).await?;
    if !accounts.contains(&account.email) {
        accounts.push(account.email.clone());
        let manifest = serde_json::to_string(&accounts).map_err(auth_error)?;
        set_secret(app, GMAIL_SECRET_SCOPE, SECRET_KEY_ACCOUNTS, &manifest).await?;
    }
    Ok(())
}

pub async fn delete_gmail_account_keyring<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    email: &str,
) -> Result<(), Error> {
    let _ = delete_secret(app, GMAIL_SECRET_SCOPE, &format!("oauth/{email}")).await;
    let _ = delete_secret(app, "google-mail/oauth", email).await;

    let mut accounts = load_gmail_account_emails(app).await?;
    accounts.retain(|e| e != email);
    let manifest = serde_json::to_string(&accounts).map_err(auth_error)?;
    set_secret(app, GMAIL_SECRET_SCOPE, SECRET_KEY_ACCOUNTS, &manifest).await?;
    Ok(())
}

pub async fn load_gmail_accounts<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
) -> Result<HashMap<String, StoredGoogleAccount>, Error> {
    let state = app.state::<GoogleAuthState>();
    let mut guard = state.gmail_accounts.lock().await;
    if let Some(cached) = guard.as_ref() {
        return Ok(cached.clone());
    }

    let emails = load_gmail_account_emails(app).await?;
    let mut map = HashMap::new();
    for email in emails {
        if let Some(account) = read_gmail_account_keyring(app, &email).await? {
            map.insert(email, account);
        }
    }

    *guard = Some(map.clone());
    Ok(map)
}

pub async fn gmail_status<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
) -> Result<GmailAccountStatus, Error> {
    let client = gmail_bundled_client();
    let has_client = client.is_some();
    let accounts = load_gmail_accounts(app).await.unwrap_or_default();
    let connected = !accounts.is_empty();

    Ok(GmailAccountStatus {
        has_client,
        connected,
        accounts: accounts.into_keys().collect(),
    })
}

pub async fn gmail_connect<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
) -> Result<GmailAccountStatus, Error> {
    let client = gmail_bundled_client().ok_or_else(|| {
        auth_error("Gmail client not configured in this build (NOTARE_GMAIL_CLIENT_ID missing)")
    })?;

    let state = app.state::<GoogleAuthState>();
    let http = state.http.clone();
    let opener_app = app.clone();

    let token = hypr_google_oauth::connect(
        &http,
        &client.client(),
        GMAIL_SCOPES,
        CONNECT_TIMEOUT,
        move |url| {
            opener_app
                .opener()
                .open_url(url, None::<&str>)
                .map_err(|e| e.to_string())
        },
    )
    .await
    .map_err(auth_error)?;

    let email = fetch_user_email(&http, &token.access_token).await?;

    let account = StoredGoogleAccount {
        email: email.clone(),
        client_id: client.client_id,
        client_secret: client.client_secret,
        auth_uri: client.auth_uri,
        token_uri: client.token_uri,
        client_kind: client.client_kind,
        refresh_token: token.refresh_token,
        granted_scopes: GMAIL_SCOPES.iter().map(|s| s.to_string()).collect(),
    };

    write_gmail_account_keyring(app, &account).await?;

    let mut accounts_guard = state.gmail_accounts.lock().await;
    if let Some(map) = &mut *accounts_guard {
        map.insert(email.clone(), account);
    } else {
        let mut map = HashMap::new();
        map.insert(email.clone(), account);
        *accounts_guard = Some(map);
    }

    if let Some(expires_in) = token.expires_in {
        let mut tokens_guard = state.gmail_tokens.lock().await;
        tokens_guard.insert(
            email,
            CachedToken {
                access_token: token.access_token,
                expires_at: Instant::now() + Duration::from_secs(expires_in),
            },
        );
    }

    gmail_status(app).await
}

pub async fn gmail_disconnect<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    email: Option<&str>,
) -> Result<GmailAccountStatus, Error> {
    let state = app.state::<GoogleAuthState>();
    let accounts = load_gmail_accounts(app).await?;

    match email {
        Some(target) => {
            if let Some(account) = accounts.get(target) {
                if let Some(rt) = &account.refresh_token {
                    let _ = hypr_google_oauth::revoke_token(
                        &state.http,
                        hypr_google_oauth::DEFAULT_REVOKE_URI,
                        rt,
                    )
                    .await;
                }
                delete_gmail_account_keyring(app, target).await?;
                let mut accounts_guard = state.gmail_accounts.lock().await;
                if let Some(map) = &mut *accounts_guard {
                    map.remove(target);
                }
                let mut tokens_guard = state.gmail_tokens.lock().await;
                tokens_guard.remove(target);
            }
        }
        None => {
            for (email, account) in accounts {
                if let Some(rt) = &account.refresh_token {
                    let _ = hypr_google_oauth::revoke_token(
                        &state.http,
                        hypr_google_oauth::DEFAULT_REVOKE_URI,
                        rt,
                    )
                    .await;
                }
                delete_gmail_account_keyring(app, &email).await?;
            }
            *state.gmail_accounts.lock().await = Some(HashMap::new());
            state.gmail_tokens.lock().await.clear();
        }
    }

    gmail_status(app).await
}

pub async fn gmail_access_token<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    account_email: Option<&str>,
) -> Result<String, Error> {
    let accounts = load_gmail_accounts(app).await?;
    let account = match account_email {
        Some(email) => accounts
            .get(email)
            .ok_or_else(|| auth_error(format!("Gmail account {email} is not connected")))?,
        None => accounts
            .values()
            .next()
            .ok_or_else(|| auth_error("no Gmail account connected"))?,
    };

    let refresh_token = account
        .refresh_token
        .clone()
        .ok_or_else(|| auth_error("no refresh token for Gmail account"))?;

    let state = app.state::<GoogleAuthState>();

    // 1. Fast path: check in-memory cache
    {
        let tokens_guard = state.gmail_tokens.lock().await;
        if let Some(cached) = tokens_guard.get(&account.email)
            && cached.expires_at.saturating_duration_since(Instant::now()) > EXPIRY_MARGIN
        {
            return Ok(cached.access_token.clone());
        }
    }

    // 2. Per-account lock to prevent concurrent double-refresh
    let refresh_lock = {
        let mut locks = state.gmail_refresh_locks.lock().unwrap();
        locks.entry(account.email.clone()).or_default().clone()
    };
    let _refresh_guard = refresh_lock.lock().await;

    // 3. Re-check cache in case another task refreshed while waiting
    {
        let tokens_guard = state.gmail_tokens.lock().await;
        if let Some(cached) = tokens_guard.get(&account.email)
            && cached.expires_at.saturating_duration_since(Instant::now()) > EXPIRY_MARGIN
        {
            return Ok(cached.access_token.clone());
        }
    }

    // 4. Refresh without holding state.gmail_tokens
    let client = account.client();
    let email = account.email.clone();
    let token = hypr_google_oauth::refresh_access_token(&state.http, &client, &refresh_token)
        .await
        .map_err(|e| {
            auth_error(format!(
                "access-token refresh failed for Gmail {email}: {e}"
            ))
        })?;

    let access_token = token.access_token.clone();
    {
        let mut tokens_guard = state.gmail_tokens.lock().await;
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

/// Assemble an RFC 2822 email message.
/// Header values (recipients and subject) are sanitized to prevent CRLF injection.
pub fn build_rfc2822_message(to: &[String], subject: &str, body: &str) -> String {
    let sanitized_to: Vec<String> = to
        .iter()
        .map(|addr| addr.replace(['\r', '\n'], ""))
        .collect();
    let to_line = sanitized_to.join(", ");
    let sanitized_subject = subject.replace(['\r', '\n'], "");
    format!(
        "To: {to_line}\r\nSubject: {sanitized_subject}\r\nContent-Type: text/plain; charset=UTF-8\r\nMIME-Version: 1.0\r\n\r\n{body}"
    )
}

/// Create a Gmail draft in the user's account (`users.drafts.create`).
pub async fn gmail_create_draft<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    account_email: Option<String>,
    to: Vec<String>,
    subject: String,
    body: String,
) -> Result<String, Error> {
    use base64::Engine;

    let token = gmail_access_token(app, account_email.as_deref()).await?;
    let rfc2822 = build_rfc2822_message(&to, &subject, &body);
    let raw_encoded = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(rfc2822.as_bytes());

    let req = hypr_google_mail::CreateDraftRequest {
        user_id: "me".to_string(),
        draft: hypr_google_mail::Draft {
            id: None,
            message: Some(hypr_google_mail::DraftMessage {
                id: None,
                thread_id: None,
                raw: Some(raw_encoded),
            }),
        },
    };

    let http = GoogleRestHttp::new("https://gmail.googleapis.com", &token)?;
    let client = hypr_google_mail::GoogleMailClient::new(http);
    let draft = client
        .create_draft(req)
        .await
        .map_err(|e| Error::Api(e.to_string()))?;

    Ok(draft.id.unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_account_key_migration() {
        let legacy_json = r#"{
            "client_id": "test-client-id",
            "client_secret": "test-client-secret",
            "auth_uri": "https://accounts.google.com/o/oauth2/auth",
            "token_uri": "https://oauth2.googleapis.com/token",
            "client_kind": "installed",
            "refresh_token": "test-refresh-token"
        }"#;

        let email = "user@example.com";
        let migrated = migrate_legacy_blob(legacy_json, email).expect("migration should succeed");

        assert_eq!(migrated.email, email);
        assert_eq!(migrated.client_id, "test-client-id");
        assert_eq!(migrated.client_secret, "test-client-secret");
        assert_eq!(
            migrated.refresh_token,
            Some("test-refresh-token".to_string())
        );
        assert_eq!(
            migrated.granted_scopes,
            DEFAULT_READ_SCOPES
                .iter()
                .map(|s| s.to_string())
                .collect::<Vec<_>>()
        );

        // Verify key formatting
        let expected_key = format!("oauth/{email}");
        assert_eq!(expected_key, "oauth/user@example.com");
    }

    #[test]
    fn test_per_account_token_selection() {
        // Connection ID parsing:
        assert_eq!(
            parse_google_connection_id("google:alice@example.com"),
            Some("alice@example.com")
        );
        assert_eq!(
            parse_google_connection_id("google:bob@company.com"),
            Some("bob@company.com")
        );
        assert_eq!(parse_google_connection_id("google-direct"), None);

        // Account map selection:
        let mut accounts = HashMap::new();
        accounts.insert(
            "alice@example.com".to_string(),
            StoredGoogleAccount {
                email: "alice@example.com".to_string(),
                client_id: "alice-client".to_string(),
                client_secret: "alice-secret".to_string(),
                auth_uri: "auth".to_string(),
                token_uri: "token".to_string(),
                client_kind: None,
                refresh_token: Some("alice-refresh-token".to_string()),
                granted_scopes: vec![],
            },
        );
        accounts.insert(
            "bob@example.com".to_string(),
            StoredGoogleAccount {
                email: "bob@example.com".to_string(),
                client_id: "bob-client".to_string(),
                client_secret: "bob-secret".to_string(),
                auth_uri: "auth".to_string(),
                token_uri: "token".to_string(),
                client_kind: None,
                refresh_token: Some("bob-refresh-token".to_string()),
                granted_scopes: vec![],
            },
        );

        let alice_email = parse_google_connection_id("google:alice@example.com").unwrap();
        let alice_acc = accounts.get(alice_email).unwrap();
        assert_eq!(
            alice_acc.refresh_token.as_deref(),
            Some("alice-refresh-token")
        );

        let bob_email = parse_google_connection_id("google:bob@example.com").unwrap();
        let bob_acc = accounts.get(bob_email).unwrap();
        assert_eq!(bob_acc.refresh_token.as_deref(), Some("bob-refresh-token"));
    }

    #[test]
    fn test_incremental_scope_url_construction() {
        let creds = ClientCredentials {
            client_id: "test-client-id-123".to_string(),
            client_secret: "secret".to_string(),
            auth_uri: "https://accounts.google.com/o/oauth2/auth".to_string(),
            token_uri: "https://oauth2.googleapis.com/token".to_string(),
        };

        let url = build_incremental_auth_url(
            &creds,
            "http://127.0.0.1:8080/callback",
            WRITE_EVENTS_SCOPE,
            "random-state-abc",
            "challenge-xyz",
        );

        assert!(url.starts_with("https://accounts.google.com/o/oauth2/auth?"));
        assert!(url.contains("client_id=test-client-id-123"));
        assert!(url.contains("redirect_uri=http%3A%2F%2F127.0.0.1%3A8080%2Fcallback"));
        assert!(url.contains("response_type=code"));
        assert!(url.contains(&format!(
            "scope={}",
            urlencoding::encode(WRITE_EVENTS_SCOPE)
        )));
        assert!(url.contains("access_type=offline"));
        assert!(url.contains("include_granted_scopes=true"));
        assert!(url.contains("prompt=consent"));
        assert!(url.contains("state=random-state-abc"));
        assert!(url.contains("code_challenge=challenge-xyz"));
        assert!(url.contains("code_challenge_method=S256"));
    }

    #[test]
    fn test_rfc2822_message_formatting() {
        let to = vec![
            "alice@example.com".to_string(),
            "bob@example.com".to_string(),
        ];
        let subject = "Follow-up: Team Sync";
        let body = "Recap:\n- Discussed Q4 roadmap\n\nAction Items:\n- Alice to review PR";

        let message = build_rfc2822_message(&to, subject, body);
        assert!(message.starts_with("To: alice@example.com, bob@example.com\r\n"));
        assert!(message.contains("Subject: Follow-up: Team Sync\r\n"));
        assert!(message.contains("Content-Type: text/plain; charset=UTF-8\r\n"));
        assert!(message.contains("MIME-Version: 1.0\r\n\r\n"));
        assert!(message.ends_with(body));
    }

    #[test]
    fn test_resolve_target_account_for_create_event() {
        let mut accounts = HashMap::new();
        accounts.insert(
            "alice@example.com".to_string(),
            StoredGoogleAccount {
                email: "alice@example.com".to_string(),
                client_id: "alice-client".to_string(),
                client_secret: "alice-secret".to_string(),
                auth_uri: "auth".to_string(),
                token_uri: "token".to_string(),
                client_kind: None,
                refresh_token: Some("alice-token".to_string()),
                granted_scopes: vec![],
            },
        );
        accounts.insert(
            "bob@example.com".to_string(),
            StoredGoogleAccount {
                email: "bob@example.com".to_string(),
                client_id: "bob-client".to_string(),
                client_secret: "bob-secret".to_string(),
                auth_uri: "auth".to_string(),
                token_uri: "token".to_string(),
                client_kind: None,
                refresh_token: Some("bob-token".to_string()),
                granted_scopes: vec![],
            },
        );

        // Resolves alice even if calendar_tracking_id is a secondary calendar or "primary"
        let alice = resolve_target_account(&accounts, "google:alice@example.com").unwrap();
        assert_eq!(alice.email, "alice@example.com");

        // Resolves bob correctly
        let bob = resolve_target_account(&accounts, "google:bob@example.com").unwrap();
        assert_eq!(bob.email, "bob@example.com");

        // Non-connected account returns error
        let err = resolve_target_account(&accounts, "google:charlie@example.com");
        assert!(err.is_err());

        // google-direct falls back to an available account
        let direct = resolve_target_account(&accounts, "google-direct").unwrap();
        assert!(direct.email == "alice@example.com" || direct.email == "bob@example.com");
    }

    #[test]
    fn test_build_rfc2822_message_header_injection() {
        let to = vec![
            "alice@example.com\r\nBcc: victim@example.com".to_string(),
            "bob@example.com\nTo-Injected: hacker@example.com".to_string(),
        ];
        let subject = "Status Update\r\nBcc: evil@example.com\r\nX-Injected: true";
        let body = "This is a clean body.";

        let message = build_rfc2822_message(&to, subject, body);
        let header_part = message.split("\r\n\r\n").next().unwrap();

        // Ensure injected headers are NOT present as individual header lines
        assert!(!header_part.contains("\r\nBcc:"));
        assert!(!header_part.contains("\nBcc:"));
        assert!(!header_part.contains("\r\nTo-Injected:"));
        assert!(!header_part.contains("\r\nX-Injected:"));
        // Verify CRLF is completely stripped from header section
        let lines: Vec<&str> = header_part.split("\r\n").collect();
        assert_eq!(lines.len(), 4); // To, Subject, Content-Type, MIME-Version
        assert_eq!(
            lines[0],
            "To: alice@example.comBcc: victim@example.com, bob@example.comTo-Injected: hacker@example.com"
        );
        assert_eq!(
            lines[1],
            "Subject: Status UpdateBcc: evil@example.comX-Injected: true"
        );
    }

    #[test]
    fn test_validate_calendar_id() {
        assert!(validate_calendar_id("primary").is_ok());
        assert!(validate_calendar_id("alice@example.com").is_ok());
        assert!(validate_calendar_id("c_188abc@group.calendar.google.com").is_ok());
        assert!(validate_calendar_id("en.usa#holiday@group.v.calendar.google.com").is_ok());

        assert!(validate_calendar_id("").is_err());
        assert!(validate_calendar_id("   ").is_err());
        assert!(validate_calendar_id("../traversal").is_err());
        assert!(validate_calendar_id("path/segment").is_err());
        assert!(validate_calendar_id("path\\segment").is_err());
        assert!(validate_calendar_id("id\nwith\rnewlines").is_err());
    }

    #[test]
    fn test_parse_event_datetime() {
        let all_day = parse_event_datetime(true, "2026-11-03").unwrap();
        assert!(all_day.date.is_some());
        assert!(all_day.date_time.is_none());

        let timed = parse_event_datetime(false, "2026-11-03T10:00:00Z").unwrap();
        assert!(timed.date.is_none());
        assert!(timed.date_time.is_some());

        // Invalid datetimes must return Err rather than silently returning None
        assert!(parse_event_datetime(true, "not-a-date").is_err());
        assert!(parse_event_datetime(false, "invalid-time").is_err());
    }
}

//! Direct ("bring your own OAuth client") Google Calendar integration.
//!
//! The user imports their own Desktop-app OAuth client json (from the Google
//! Cloud console), we run the installed-app loopback flow via
//! `hypr_google_oauth`, and persist `{client_id, client_secret, refresh_token}`
//! in the OS keyring through the store2 secure-store. Access tokens are cached
//! in memory and refreshed automatically.

use std::time::{Duration, Instant};

use hypr_google_oauth::{ClientCredentials, ClientJsonKind};
use tauri::Manager;
use tauri_plugin_opener::OpenerExt;

use crate::error::Error;

/// Secure-store (OS keyring) location for the credentials blob.
const SECRET_SCOPE: &str = "google-calendar";
const SECRET_KEY: &str = "oauth";

/// How long we wait for the user to finish the browser consent flow.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(300);
/// Refresh the access token this long before it actually expires.
const EXPIRY_MARGIN: Duration = Duration::from_secs(60);

/// What gets serialized into the keyring entry.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StoredGoogleCredentials {
    pub client_id: String,
    pub client_secret: String,
    pub auth_uri: String,
    pub token_uri: String,
    /// `"installed"` or `"web"`.
    #[serde(default)]
    pub client_kind: Option<String>,
    #[serde(default)]
    pub refresh_token: Option<String>,
}

impl StoredGoogleCredentials {
    fn client(&self) -> ClientCredentials {
        ClientCredentials {
            client_id: self.client_id.clone(),
            client_secret: self.client_secret.clone(),
            auth_uri: self.auth_uri.clone(),
            token_uri: self.token_uri.clone(),
        }
    }
}

/// The bundled first-party OAuth client (Notare's own GCP project), baked in at
/// build time so users can "Sign in with Google" without creating their own
/// Google Cloud project. Present only when the build was given the creds (via the
/// `NOTARE_GOOGLE_CLIENT_ID`/`_SECRET` env at compile time); absent in dev/fork
/// builds, where the app falls back to a user-imported (BYO) client. A Desktop
/// OAuth client secret is not confidential (loopback + PKCE is the protection),
/// so baking it in is the standard approach — mirrors POSTHOG_API_KEY etc.
fn bundled_client() -> Option<StoredGoogleCredentials> {
    let client_id = option_env!("NOTARE_GOOGLE_CLIENT_ID").filter(|s| !s.is_empty())?;
    let client_secret = option_env!("NOTARE_GOOGLE_CLIENT_SECRET").filter(|s| !s.is_empty())?;
    Some(StoredGoogleCredentials {
        client_id: client_id.to_string(),
        client_secret: client_secret.to_string(),
        auth_uri: "https://accounts.google.com/o/oauth2/auth".to_string(),
        token_uri: "https://oauth2.googleapis.com/token".to_string(),
        client_kind: Some("installed".to_string()),
        refresh_token: None,
    })
}

/// Status surfaced to the frontend.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct GoogleAccountStatus {
    /// A client json has been imported (BYO client).
    pub has_client: bool,
    /// A bundled first-party client is compiled into this build, so the user can
    /// "Sign in with Google" without importing their own client.
    pub has_bundled_client: bool,
    /// A refresh token exists (i.e. the consent flow completed).
    pub connected: bool,
    /// `"installed"` or `"web"` (when a client is present).
    pub client_kind: Option<String>,
    /// The client id, so the user can tell which GCP client is in use.
    pub client_id: Option<String>,
}

/// Result of importing a client json.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct GoogleClientImportResult {
    pub client_id: String,
    /// `"installed"` or `"web"`.
    pub client_kind: String,
    /// True when the json was a "web" client — loopback redirects may not
    /// work with those; the UI should warn but proceed.
    pub warning_web_client: bool,
}

struct CachedToken {
    access_token: String,
    expires_at: Instant,
}

/// Managed state: in-memory credential + access-token cache so the OS keyring
/// is only touched on first use and on mutations.
pub struct GoogleAuthState {
    http: reqwest::Client,
    /// `None` = not loaded from keyring yet; `Some(None)` = loaded, absent.
    creds: tokio::sync::Mutex<Option<Option<StoredGoogleCredentials>>>,
    token: tokio::sync::Mutex<Option<CachedToken>>,
}

impl Default for GoogleAuthState {
    fn default() -> Self {
        Self {
            http: reqwest::Client::new(),
            creds: tokio::sync::Mutex::new(None),
            token: tokio::sync::Mutex::new(None),
        }
    }
}

fn auth_error(message: impl std::fmt::Display) -> Error {
    Error::Auth(format!("google: {message}"))
}

async fn read_keyring<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
) -> Result<Option<StoredGoogleCredentials>, Error> {
    let app = app.clone();
    let raw = tauri::async_runtime::spawn_blocking(move || {
        tauri_plugin_store2::secrets::get_secret_blocking(&app, SECRET_SCOPE, SECRET_KEY)
    })
    .await
    .map_err(auth_error)?
    .map_err(auth_error)?;

    match raw {
        None => Ok(None),
        Some(raw) => serde_json::from_str(&raw).map(Some).map_err(|e| {
            auth_error(format!(
                "stored credentials are corrupt ({e}); re-import the client json"
            ))
        }),
    }
}

async fn write_keyring<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    creds: &StoredGoogleCredentials,
) -> Result<(), Error> {
    let app = app.clone();
    let value = serde_json::to_string(creds).map_err(auth_error)?;
    tauri::async_runtime::spawn_blocking(move || {
        tauri_plugin_store2::secrets::set_secret_blocking(&app, SECRET_SCOPE, SECRET_KEY, &value)
    })
    .await
    .map_err(auth_error)?
    .map_err(auth_error)
}

async fn delete_keyring<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> Result<(), Error> {
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        tauri_plugin_store2::secrets::delete_secret_blocking(&app, SECRET_SCOPE, SECRET_KEY)
    })
    .await
    .map_err(auth_error)?
    .map_err(auth_error)
}

/// Load credentials (memoized).
async fn load<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
) -> Result<Option<StoredGoogleCredentials>, Error> {
    let state = app.state::<GoogleAuthState>();
    let mut guard = state.creds.lock().await;
    if let Some(cached) = guard.as_ref() {
        return Ok(cached.clone());
    }
    let loaded = read_keyring(app).await?;
    *guard = Some(loaded.clone());
    Ok(loaded)
}

async fn store<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    creds: StoredGoogleCredentials,
) -> Result<(), Error> {
    write_keyring(app, &creds).await?;
    let state = app.state::<GoogleAuthState>();
    *state.creds.lock().await = Some(Some(creds));
    Ok(())
}

async fn clear_token_cache<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    let state = app.state::<GoogleAuthState>();
    *state.token.lock().await = None;
}

/// Import a client json (pasted text or file contents).
pub async fn import_client_json<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    json: &str,
) -> Result<GoogleClientImportResult, Error> {
    let (client, kind) = hypr_google_oauth::parse_client_json(json).map_err(auth_error)?;

    let kind_str = match kind {
        ClientJsonKind::Installed => "installed",
        ClientJsonKind::Web => "web",
    };

    // A new client invalidates any previous session.
    let creds = StoredGoogleCredentials {
        client_id: client.client_id.clone(),
        client_secret: client.client_secret,
        auth_uri: client.auth_uri,
        token_uri: client.token_uri,
        client_kind: Some(kind_str.to_string()),
        refresh_token: None,
    };
    store(app, creds).await?;
    clear_token_cache(app).await;

    Ok(GoogleClientImportResult {
        client_id: client.client_id,
        client_kind: kind_str.to_string(),
        warning_web_client: kind == ClientJsonKind::Web,
    })
}

/// Import a client json from a file path (the file-picker flow).
pub async fn import_client_file<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    path: &str,
) -> Result<GoogleClientImportResult, Error> {
    let json = tokio::fs::read_to_string(path)
        .await
        .map_err(|e| auth_error(format!("could not read {path}: {e}")))?;
    import_client_json(app, &json).await
}

pub async fn status<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
) -> Result<GoogleAccountStatus, Error> {
    let creds = load(app).await?;
    let has_bundled_client = bundled_client().is_some();
    Ok(match creds {
        None => GoogleAccountStatus {
            has_client: false,
            has_bundled_client,
            connected: false,
            client_kind: None,
            client_id: None,
        },
        Some(creds) => GoogleAccountStatus {
            has_client: true,
            has_bundled_client,
            connected: creds.refresh_token.is_some(),
            client_kind: creds.client_kind.clone(),
            client_id: Some(creds.client_id.clone()),
        },
    })
}

/// Cheap connected-check used by the provider plumbing.
pub async fn is_connected<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> bool {
    matches!(
        load(app).await,
        Ok(Some(StoredGoogleCredentials {
            refresh_token: Some(_),
            ..
        }))
    )
}

/// Run the full browser consent flow and persist the refresh token.
pub async fn connect<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
) -> Result<GoogleAccountStatus, Error> {
    // Prefer a user-imported (BYO) client; otherwise fall back to the bundled
    // first-party client so users don't need their own Google Cloud project.
    let mut creds = match load(app).await? {
        Some(creds) => creds,
        None => bundled_client().ok_or_else(|| {
            auth_error("no Google OAuth client available — add your own under Advanced settings")
        })?,
    };

    let state = app.state::<GoogleAuthState>();
    let http = state.http.clone();

    let opener_app = app.clone();
    let token = hypr_google_oauth::connect(
        &http,
        &creds.client(),
        hypr_google_oauth::DEFAULT_SCOPES,
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

    creds.refresh_token = token.refresh_token.clone();
    store(app, creds).await?;

    // Prime the access-token cache with the token we just received.
    if let Some(expires_in) = token.expires_in {
        let state = app.state::<GoogleAuthState>();
        *state.token.lock().await = Some(CachedToken {
            access_token: token.access_token,
            expires_at: Instant::now() + Duration::from_secs(expires_in),
        });
    }

    status(app).await
}

/// Drop the refresh token (and revoke it, best-effort). Keeps the client json
/// so the user can reconnect without re-importing.
pub async fn disconnect<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
) -> Result<GoogleAccountStatus, Error> {
    if let Some(mut creds) = load(app).await? {
        if let Some(refresh_token) = creds.refresh_token.take() {
            let state = app.state::<GoogleAuthState>();
            let http = state.http.clone();
            if let Err(e) = hypr_google_oauth::revoke_token(
                &http,
                hypr_google_oauth::DEFAULT_REVOKE_URI,
                &refresh_token,
            )
            .await
            {
                tracing::warn!("failed to revoke google token (continuing): {e}");
            }
        }
        store(app, creds).await?;
    }
    clear_token_cache(app).await;
    status(app).await
}

/// Remove everything: refresh token AND the imported client json.
pub async fn reset<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
) -> Result<GoogleAccountStatus, Error> {
    // Revoke first (best-effort) while we still have the token.
    let _ = disconnect(app).await;
    delete_keyring(app).await?;
    let state = app.state::<GoogleAuthState>();
    *state.creds.lock().await = Some(None);
    clear_token_cache(app).await;
    status(app).await
}

/// Get a valid Google access token, refreshing if needed.
pub async fn access_token<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> Result<String, Error> {
    let Some(creds) = load(app).await? else {
        return Err(hypr_calendar::Error::NotAuthenticated.into());
    };
    let Some(refresh_token) = creds.refresh_token.clone() else {
        return Err(hypr_calendar::Error::NotAuthenticated.into());
    };

    let state = app.state::<GoogleAuthState>();
    let mut guard = state.token.lock().await;

    if let Some(cached) = guard.as_ref() {
        if cached.expires_at.saturating_duration_since(Instant::now()) > EXPIRY_MARGIN {
            return Ok(cached.access_token.clone());
        }
    }

    let token =
        hypr_google_oauth::refresh_access_token(&state.http, &creds.client(), &refresh_token)
            .await
            .map_err(|e| {
                auth_error(format!(
                    "access-token refresh failed ({e}); try reconnecting Google Calendar"
                ))
            })?;

    // Google occasionally rotates the refresh token.
    if let Some(new_refresh) = token.refresh_token.clone() {
        if new_refresh != refresh_token {
            let mut updated = creds.clone();
            updated.refresh_token = Some(new_refresh);
            drop(guard);
            store(app, updated).await?;
            guard = state.token.lock().await;
        }
    }

    let access_token = token.access_token.clone();
    *guard = Some(CachedToken {
        access_token: token.access_token,
        expires_at: Instant::now() + Duration::from_secs(token.expires_in.unwrap_or(3600)),
    });

    Ok(access_token)
}

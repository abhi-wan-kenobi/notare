use hypr_llm_auth::{AccessTokenResult, AuthStatus, CatalogResult, ProviderId, StartFlowResult};
use tauri::Manager;

use crate::LlmAuthState;
use crate::error::Error;

fn get_state<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
) -> Result<tauri::State<'_, LlmAuthState<R>>, Error> {
    app.try_state::<LlmAuthState<R>>()
        .ok_or_else(|| Error::Auth("llm-auth not initialized".into()))
}

#[tauri::command]
#[specta::specta]
pub async fn status<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    provider: ProviderId,
) -> Result<AuthStatus, Error> {
    let state = get_state(&app)?;
    state.manager.status(provider).await.map_err(Into::into)
}

#[tauri::command]
#[specta::specta]
pub async fn start<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    provider: ProviderId,
) -> Result<StartFlowResult, Error> {
    let state = get_state(&app)?;
    state.manager.start(provider).await.map_err(Into::into)
}

#[tauri::command]
#[specta::specta]
pub async fn complete<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    provider: ProviderId,
    pasted_code: Option<String>,
) -> Result<AuthStatus, Error> {
    let state = get_state(&app)?;
    state
        .manager
        .complete(provider, pasted_code)
        .await
        .map_err(Into::into)
}

#[tauri::command]
#[specta::specta]
pub async fn cancel<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    provider: ProviderId,
) -> Result<(), Error> {
    let state = get_state(&app)?;
    state.manager.cancel(provider).await.map_err(Into::into)
}

#[tauri::command]
#[specta::specta]
pub async fn logout<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    provider: ProviderId,
) -> Result<(), Error> {
    let state = get_state(&app)?;
    state.manager.logout(provider).await.map_err(Into::into)
}

#[tauri::command]
#[specta::specta]
pub async fn access_token<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    provider: ProviderId,
) -> Result<AccessTokenResult, Error> {
    let state = get_state(&app)?;
    state
        .manager
        .access_token(provider)
        .await
        .map_err(Into::into)
}

#[tauri::command]
#[specta::specta]
pub async fn catalog<R: tauri::Runtime>(app: tauri::AppHandle<R>) -> Result<CatalogResult, Error> {
    let state = get_state(&app)?;
    state.manager.catalog().await.map_err(Into::into)
}

/// Where the settings UI keeps API keys (`plugins/store2` secure-store).
/// Must match `PROVIDER_SECRET_SCOPE` and `providerRowId("llm", "openrouter")`
/// in `apps/desktop/src/settings/providers.ts`.
const PROVIDER_SECRET_SCOPE: &str = "ai-provider-api-keys";
const OPENROUTER_SECRET_KEY: &str = "llm:openrouter";

/// Browser sign-in with OpenRouter. The API key goes from the token exchange
/// straight into the keyring; it is never returned to the webview.
#[tauri::command]
#[specta::specta]
pub async fn openrouter_sign_in<R: tauri::Runtime>(app: tauri::AppHandle<R>) -> Result<(), Error> {
    use tauri_plugin_opener2::Opener2PluginExt;

    let state = get_state(&app)?;
    let (cancel_tx, cancel_rx) = tokio::sync::oneshot::channel::<()>();
    *state
        .openrouter_cancel
        .lock()
        .unwrap_or_else(|e| e.into_inner()) = Some(cancel_tx);
    let manager = state.manager.clone();

    let opener_app = app.clone();
    let key = manager
        .openrouter_sign_in(
            move |url| {
                opener_app
                    .opener2()
                    .open_url(&url, None)
                    .map_err(|e| e.to_string())
            },
            async move {
                // Both an explicit cancel and a newer sign-in dropping the
                // sender end this flow.
                let _ = cancel_rx.await;
            },
        )
        .await?;

    tauri::async_runtime::spawn_blocking(move || {
        tauri_plugin_store2::secrets::set_secret_blocking(
            &app,
            PROVIDER_SECRET_SCOPE,
            OPENROUTER_SECRET_KEY,
            &key,
        )
    })
    .await
    .map_err(|e| Error::Auth(e.to_string()))?
    .map_err(Error::Auth)
}

#[tauri::command]
#[specta::specta]
pub async fn openrouter_cancel<R: tauri::Runtime>(app: tauri::AppHandle<R>) -> Result<(), Error> {
    let state = get_state(&app)?;
    let tx = state
        .openrouter_cancel
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .take();
    if let Some(tx) = tx {
        let _ = tx.send(());
    }
    Ok(())
}

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

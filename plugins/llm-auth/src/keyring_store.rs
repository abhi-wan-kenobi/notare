use hypr_llm_auth::{KEYRING_SCOPE, ProviderId, StoredTokens, TokenStore};
use std::future::Future;

pub struct KeyringTokenStore<R: tauri::Runtime> {
    app: tauri::AppHandle<R>,
}

impl<R: tauri::Runtime> KeyringTokenStore<R> {
    pub fn new(app: tauri::AppHandle<R>) -> Self {
        Self { app }
    }
}

impl<R: tauri::Runtime> TokenStore for KeyringTokenStore<R> {
    fn get(
        &self,
        provider: ProviderId,
    ) -> impl Future<Output = Result<Option<StoredTokens>, hypr_llm_auth::Error>> + Send {
        let app = self.app.clone();
        let key = provider.as_str().to_string();

        async move {
            let app_get = app.clone();
            let key_get = key.clone();
            let raw = tauri::async_runtime::spawn_blocking(move || {
                tauri_plugin_store2::secrets::get_secret_blocking(&app_get, KEYRING_SCOPE, &key_get)
            })
            .await
            .map_err(|e| hypr_llm_auth::Error::Store(e.to_string()))?
            .map_err(hypr_llm_auth::Error::Store)?;

            match raw {
                Some(s) => match serde_json::from_str(&s) {
                    Ok(tokens) => Ok(Some(tokens)),
                    Err(e) => {
                        tracing::warn!(
                            "Corrupted tokens in keyring for provider {}, clearing: {}",
                            provider.as_str(),
                            e
                        );
                        let _ = tauri::async_runtime::spawn_blocking(move || {
                            tauri_plugin_store2::secrets::delete_secret_blocking(
                                &app,
                                KEYRING_SCOPE,
                                &key,
                            )
                        })
                        .await;
                        Ok(None)
                    }
                },
                None => Ok(None),
            }
        }
    }

    fn set(
        &self,
        provider: ProviderId,
        tokens: &StoredTokens,
    ) -> impl Future<Output = Result<(), hypr_llm_auth::Error>> + Send {
        let app = self.app.clone();
        let key = provider.as_str().to_string();
        let tokens_res = serde_json::to_string(tokens);

        async move {
            let value = tokens_res.map_err(|e| hypr_llm_auth::Error::Store(e.to_string()))?;
            tauri::async_runtime::spawn_blocking(move || {
                tauri_plugin_store2::secrets::set_secret_blocking(&app, KEYRING_SCOPE, &key, &value)
            })
            .await
            .map_err(|e| hypr_llm_auth::Error::Store(e.to_string()))?
            .map_err(hypr_llm_auth::Error::Store)
        }
    }

    fn delete(
        &self,
        provider: ProviderId,
    ) -> impl Future<Output = Result<(), hypr_llm_auth::Error>> + Send {
        let app = self.app.clone();
        let key = provider.as_str().to_string();

        async move {
            tauri::async_runtime::spawn_blocking(move || {
                tauri_plugin_store2::secrets::delete_secret_blocking(&app, KEYRING_SCOPE, &key)
            })
            .await
            .map_err(|e| hypr_llm_auth::Error::Store(e.to_string()))?
            .map_err(hypr_llm_auth::Error::Store)
        }
    }
}

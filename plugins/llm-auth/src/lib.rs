mod commands;
mod error;
mod keyring_store;

use hypr_llm_auth::LlmAuthManager;
use std::sync::Arc;

pub use error::Error;
pub use keyring_store::KeyringTokenStore;

pub struct LlmAuthState<R: tauri::Runtime> {
    pub manager: Arc<LlmAuthManager<KeyringTokenStore<R>>>,
}

const PLUGIN_NAME: &str = "llm-auth";

fn make_specta_builder<R: tauri::Runtime>() -> tauri_specta::Builder<R> {
    tauri_specta::Builder::<R>::new()
        .plugin_name(PLUGIN_NAME)
        .commands(tauri_specta::collect_commands![
            commands::status::<tauri::Wry>,
            commands::start::<tauri::Wry>,
            commands::complete::<tauri::Wry>,
            commands::cancel::<tauri::Wry>,
            commands::logout::<tauri::Wry>,
            commands::access_token::<tauri::Wry>,
            commands::catalog::<tauri::Wry>,
        ])
        .error_handling(tauri_specta::ErrorHandlingMode::Result)
}

pub fn init<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    let specta_builder = make_specta_builder();

    tauri::plugin::Builder::new(PLUGIN_NAME)
        .invoke_handler(specta_builder.invoke_handler())
        .setup(|app, _api| {
            use tauri::Manager;
            let store = Arc::new(KeyringTokenStore::new(app.clone()));
            let manager = Arc::new(LlmAuthManager::new(store));
            app.manage(LlmAuthState { manager });
            Ok(())
        })
        .build()
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn export_types() {
        const OUTPUT_FILE: &str = "./js/bindings.gen.ts";

        make_specta_builder::<tauri::Wry>()
            .export(
                specta_typescript::Typescript::default()
                    .formatter(specta_typescript::formatter::prettier)
                    .bigint(specta_typescript::BigIntExportBehavior::Number),
                OUTPUT_FILE,
            )
            .unwrap();

        let content = std::fs::read_to_string(OUTPUT_FILE).unwrap();
        std::fs::write(OUTPUT_FILE, format!("// @ts-nocheck\n{content}")).unwrap();
    }
}

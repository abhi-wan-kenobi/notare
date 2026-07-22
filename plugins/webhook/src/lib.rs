mod commands;
pub mod delivery;
mod error;
mod ext;
mod openapi;
pub mod types;

pub use error::*;
pub use ext::*;
pub use openapi::*;

use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::Duration;

const PLUGIN_NAME: &str = "webhook";

use tauri::Manager;

/// Plugin runtime state: a shared HTTP client and the in-memory delivery log.
pub struct State {
    pub client: reqwest::Client,
    pub log: Mutex<VecDeque<types::DeliveryRecord>>,
}

impl Default for State {
    fn default() -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(15))
            .connect_timeout(Duration::from_secs(10))
            .user_agent(concat!("notare-webhook/", env!("CARGO_PKG_VERSION")))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        Self {
            client,
            log: Mutex::new(VecDeque::with_capacity(types::DELIVERY_LOG_CAP)),
        }
    }
}

fn make_specta_builder() -> tauri_specta::Builder<tauri::Wry> {
    tauri_specta::Builder::<tauri::Wry>::new()
        .plugin_name(PLUGIN_NAME)
        .events(tauri_specta::collect_events![])
        .commands(tauri_specta::collect_commands![
            commands::get_settings::<tauri::Wry>,
            commands::set_settings::<tauri::Wry>,
            commands::set_secret::<tauri::Wry>,
            commands::clear_secret::<tauri::Wry>,
            commands::recent_deliveries::<tauri::Wry>,
            commands::send_webhook::<tauri::Wry>,
            commands::test_webhook::<tauri::Wry>,
        ])
        .error_handling(tauri_specta::ErrorHandlingMode::Result)
}

pub fn init() -> tauri::plugin::TauriPlugin<tauri::Wry> {
    let specta_builder = make_specta_builder();

    tauri::plugin::Builder::new(PLUGIN_NAME)
        .invoke_handler(specta_builder.invoke_handler())
        .setup(move |app, _api| {
            specta_builder.mount_events(app);

            {
                app.manage(State::default());
            }

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

        make_specta_builder()
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

    #[test]
    fn export_openapi() {
        let openapi_json = generate_openapi_json();
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("openapi.gen.json");
        std::fs::write(&path, openapi_json).unwrap();
    }
}

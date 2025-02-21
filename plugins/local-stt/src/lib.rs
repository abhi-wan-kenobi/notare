use tauri::{Manager, Wry};

mod commands;
mod error;
mod ext;
mod model;
mod server;

pub use error::{Error, Result};
pub use ext::LocalSttPluginExt;

pub type SharedState = std::sync::Arc<std::sync::Mutex<State>>;

#[derive(Default)]
pub struct State {
    pub api_base: String,
    pub model: Option<rwhisper::Whisper>,
    pub server: Option<crate::server::ServerHandle>,
}

const PLUGIN_NAME: &str = "local-stt";

fn make_specta_builder() -> tauri_specta::Builder<Wry> {
    tauri_specta::Builder::<Wry>::new()
        .plugin_name(PLUGIN_NAME)
        .commands(tauri_specta::collect_commands![
            commands::load_model::<Wry>,
            commands::unload_model::<Wry>,
            commands::start_server::<Wry>,
            commands::stop_server::<Wry>,
        ])
        .error_handling(tauri_specta::ErrorHandlingMode::Throw)
}

pub fn init() -> tauri::plugin::TauriPlugin<Wry> {
    let specta_builder = make_specta_builder();

    tauri::plugin::Builder::new(PLUGIN_NAME)
        .invoke_handler(specta_builder.invoke_handler())
        .setup(|app, _api| {
            app.manage(SharedState::default());
            Ok(())
        })
        .build()
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn export_types() {
        make_specta_builder()
            .export(
                specta_typescript::Typescript::default()
                    .header("// @ts-nocheck\n\n")
                    .formatter(specta_typescript::formatter::prettier)
                    .bigint(specta_typescript::BigIntExportBehavior::Number),
                "./js/bindings.gen.ts",
            )
            .unwrap()
    }
}

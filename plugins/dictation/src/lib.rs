mod clean;
mod commands;
mod error;
mod events;
mod ext;
mod handler;
mod inject;
mod orb;
mod session;

pub use error::*;
pub use events::*;
pub use ext::*;

use handler::Handler;
use tauri::Manager;

const PLUGIN_NAME: &str = "dictation";

fn make_specta_builder<R: tauri::Runtime>() -> tauri_specta::Builder<R> {
    tauri_specta::Builder::<R>::new()
        .plugin_name(PLUGIN_NAME)
        .commands(tauri_specta::collect_commands![
            commands::show::<tauri::Wry>,
            commands::hide::<tauri::Wry>,
            commands::set_phase::<tauri::Wry>,
            commands::update_amplitude::<tauri::Wry>,
            commands::show_orb::<tauri::Wry>,
            commands::hide_orb::<tauri::Wry>,
            commands::start_dictation::<tauri::Wry>,
            commands::stop_dictation::<tauri::Wry>,
            commands::is_dictating::<tauri::Wry>,
            commands::type_text::<tauri::Wry>,
            commands::deliver_text::<tauri::Wry>,
            commands::clean_text::<tauri::Wry>,
        ])
        .events(tauri_specta::collect_events![
            DictationStateEvent,
            DictationOrbClicked,
            DictationTranscriptEvent,
            DictationFinishedEvent
        ])
        .error_handling(tauri_specta::ErrorHandlingMode::Result)
}

pub fn init() -> tauri::plugin::TauriPlugin<tauri::Wry> {
    let specta_builder = make_specta_builder();

    tauri::plugin::Builder::new(PLUGIN_NAME)
        .invoke_handler(specta_builder.invoke_handler())
        .setup(move |app, _api| {
            specta_builder.mount_events(app);
            app.manage(Handler::new());
            app.manage(session::SessionState::default());
            orb::set_app_handle(app.clone());
            setup_shortcut_bridge(app);
            Ok(())
        })
        .build()
}

/// Bridges the shortcut plugin's macOS-only native push-to-talk event-tap
/// (`ShortcutEvent`) to the native overlay panel (`Handler::show`/`hide`,
/// `hypr-dictation-ui-macos`). Gated off since #31: nothing calls
/// `register_hotkey` to start that tap (the dictation orb uses the toggle
/// `register_global_hotkey` path on every platform instead), and leaving
/// this wired risked a second, native orb appearing alongside the webview
/// orb if that ever changed. The native `Handler`/panel plumbing is kept
/// in place (harmless while unreachable) rather than deleted outright.
#[cfg(not(target_os = "macos"))]
fn setup_shortcut_bridge(app: &tauri::AppHandle) {
    use ext::DictationPluginExt;
    use tauri_plugin_shortcut::ShortcutEvent;
    use tauri_specta::Event;

    let handle = app.clone();
    ShortcutEvent::listen(app, move |event| {
        let d = handle.dictation();
        match event.payload {
            ShortcutEvent::Pressed => {
                let _ = d.set_phase(Phase::Recording);
                let _ = d.show();
            }
            ShortcutEvent::Released => {
                let _ = d.set_phase(Phase::Processing);
                let _ = d.hide();
            }
            ShortcutEvent::Cancelled | ShortcutEvent::Discarded => {
                let _ = d.hide();
            }
        }
    });
}

#[cfg(target_os = "macos")]
fn setup_shortcut_bridge(_app: &tauri::AppHandle) {}

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

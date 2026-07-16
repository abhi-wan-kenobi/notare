use crate::{
    events::{DictationOutputMode, Phase},
    ext::DictationPluginExt,
};

#[tauri::command]
#[specta::specta]
pub(crate) async fn show<R: tauri::Runtime>(app: tauri::AppHandle<R>) -> Result<(), String> {
    app.dictation().show().map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn hide<R: tauri::Runtime>(app: tauri::AppHandle<R>) -> Result<(), String> {
    app.dictation().hide().map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn set_phase<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    phase: Phase,
) -> Result<(), String> {
    app.dictation().set_phase(phase).map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn update_amplitude<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    amplitude: f32,
) -> Result<(), String> {
    app.dictation()
        .update_amplitude(amplitude)
        .map_err(|e| e.to_string())
}

/// Show the persistent dictation orb window (Windows/Linux webview path;
/// unsupported on macOS, which keeps its native mini-panel).
#[tauri::command]
#[specta::specta]
pub(crate) async fn show_orb<R: tauri::Runtime>(app: tauri::AppHandle<R>) -> Result<(), String> {
    app.dictation().show_orb().map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn hide_orb<R: tauri::Runtime>(app: tauri::AppHandle<R>) -> Result<(), String> {
    app.dictation().hide_orb().map_err(|e| e.to_string())
}

/// Start a dictation session against the local STT server. `base_url` is the
/// port-bearing URL returned by the local-stt plugin (`http://127.0.0.1:<port>/v1`),
/// `model` the currently selected live STT model and `output_mode` where the
/// recognized text goes (typed live vs. paste-on-stop).
#[tauri::command]
#[specta::specta]
pub(crate) async fn start_dictation<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    base_url: String,
    model: String,
    output_mode: DictationOutputMode,
) -> Result<(), String> {
    app.dictation()
        .start_dictation(base_url, model, output_mode)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn stop_dictation<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<(), String> {
    app.dictation().stop_dictation().map_err(|e| e.to_string())
}

/// Whether a dictation session is currently running (listening/processing).
#[tauri::command]
#[specta::specta]
pub(crate) async fn is_dictating<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<bool, String> {
    app.dictation().is_dictating().map_err(|e| e.to_string())
}

/// Inject text into the currently focused app. Exposed mainly so the flow can
/// be exercised without a live STT session (devtools/testing).
#[tauri::command]
#[specta::specta]
pub(crate) async fn type_text<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    text: String,
) -> Result<(), String> {
    app.dictation()
        .type_text(text)
        .await
        .map_err(|e| e.to_string())
}

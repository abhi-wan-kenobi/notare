use crate::{
    ActivityCapturePluginExt,
    events::{
        ActivityCaptureBudget, ActivityCaptureCapabilities, ActivityCaptureScreenshotAnalysis,
        ActivityCaptureSnapshot, ActivityCaptureStatus,
    },
};

#[tauri::command]
#[specta::specta]
pub(crate) async fn capabilities<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<ActivityCaptureCapabilities, String> {
    Ok(app.activity_capture().capabilities())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn snapshot<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<Option<ActivityCaptureSnapshot>, String> {
    app.activity_capture()
        .snapshot()
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn latest_screenshot_analysis<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<Option<ActivityCaptureScreenshotAnalysis>, String> {
    Ok(app.activity_capture().latest_screenshot_analysis())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn status<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<ActivityCaptureStatus, String> {
    Ok(app.activity_capture().status().await)
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn list_analyses_in_range<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    start_ms: i64,
    end_ms: i64,
) -> Result<Vec<ActivityCaptureScreenshotAnalysis>, String> {
    app.activity_capture()
        .list_analyses_in_range(start_ms, end_ms)
        .await
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn start<R: tauri::Runtime>(app: tauri::AppHandle<R>) -> Result<(), String> {
    app.activity_capture()
        .start()
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn stop<R: tauri::Runtime>(app: tauri::AppHandle<R>) -> Result<(), String> {
    app.activity_capture().stop();
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn is_running<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<bool, String> {
    Ok(app.activity_capture().is_running())
}

#[derive(Debug, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ConfigureInput {
    pub budget: Option<ActivityCaptureBudget>,
    pub analyze_screenshots: Option<bool>,
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn configure<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    input: ConfigureInput,
) -> Result<(), String> {
    app.activity_capture()
        .configure(input.budget, input.analyze_screenshots)
        .map_err(|error| error.to_string())
}

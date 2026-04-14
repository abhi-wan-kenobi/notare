use std::path::{Path, PathBuf};

use hypr_activity_capture::ObservationScreenshotCapture;
use hypr_screen_core::CaptureSubject;
use reqwest::Client;
use serde_json::json;
use tauri_plugin_local_llm::LocalLlmPluginExt;
use url::Url;

use crate::events::{
    ActivityCaptureObservationAnalysis, ActivityCaptureObservationAnalysisError, unix_ms_now,
};

pub const ANALYSIS_MODEL_NAME: &str = "local-llm";
pub const ANALYSIS_PROMPT_VERSION: &str = "observation-v1";

struct ScreenshotAnalysisRequest {
    system_prompt: String,
    user_prompt: String,
    image_url: String,
    _temp_image: TempScreenshotFile,
}

pub async fn analyze_screenshot<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    screenshot_id: &str,
    screenshot: &ObservationScreenshotCapture,
) -> Result<ActivityCaptureObservationAnalysis, ActivityCaptureObservationAnalysisError> {
    let (app_name, window_title) = analysis_identity(screenshot);
    let server_url = app
        .local_llm()
        .server_url()
        .await
        .map_err(|error| {
            analysis_error(
                screenshot_id,
                screenshot,
                &app_name,
                window_title.clone(),
                error.to_string(),
            )
        })?
        .ok_or_else(|| {
            analysis_error(
                screenshot_id,
                screenshot,
                &app_name,
                window_title.clone(),
                "local-llm server is unavailable".to_string(),
            )
        })?;

    let request = build_request(screenshot, &app_name, window_title.clone()).map_err(|error| {
        analysis_error(
            screenshot_id,
            screenshot,
            &app_name,
            window_title.clone(),
            error,
        )
    })?;

    let summary = call_local_llm(&server_url, &request)
        .await
        .map_err(|error| {
            analysis_error(
                screenshot_id,
                screenshot,
                &app_name,
                window_title.clone(),
                error,
            )
        })?;

    Ok(ActivityCaptureObservationAnalysis {
        observation_id: screenshot.observation_id.clone(),
        screenshot_id: screenshot_id.to_string(),
        screenshot_kind: screenshot.kind.as_str().to_string(),
        captured_at_ms: screenshot.captured_at_ms,
        app_name,
        window_title,
        summary,
    })
}

fn build_request(
    screenshot: &ObservationScreenshotCapture,
    app_name: &str,
    window_title: Option<String>,
) -> Result<ScreenshotAnalysisRequest, String> {
    let system_prompt = "You are summarizing a single desktop activity observation. Use the screenshot and provided metadata to describe the user's likely task in 2-4 concise sentences. Prefer concrete UI evidence. If visible text is present, use it as grounding rather than repeating it verbatim.".to_string();
    let user_prompt = build_prompt(screenshot, app_name, window_title);
    let temp_image = TempScreenshotFile::create(screenshot)?;
    let image_url = file_url(temp_image.path())?;

    Ok(ScreenshotAnalysisRequest {
        system_prompt,
        user_prompt,
        image_url,
        _temp_image: temp_image,
    })
}

fn build_prompt(
    screenshot: &ObservationScreenshotCapture,
    app_name: &str,
    window_title: Option<String>,
) -> String {
    let snapshot = &screenshot.snapshot;
    let text_excerpt = snapshot.primary_text().unwrap_or_default();
    [
        format!("Observation ID: {}", screenshot.observation_id),
        format!("Screenshot kind: {}", screenshot.kind.as_str()),
        format!("App: {app_name}"),
        format!("Window title: {}", window_title.unwrap_or_default()),
        format!("Activity kind: {}", snapshot.activity_kind.as_str()),
        format!("URL: {}", snapshot.url.clone().unwrap_or_default()),
        format!(
            "Text anchor identity: {}",
            snapshot.text_anchor_identity.clone().unwrap_or_default()
        ),
        format!("Text excerpt: {text_excerpt}"),
        "Describe the user's likely task, not just the visible controls.".to_string(),
    ]
    .join("\n")
}

async fn call_local_llm(
    server_url: &str,
    request: &ScreenshotAnalysisRequest,
) -> Result<String, String> {
    let response = Client::new()
        .post(format!("{server_url}/chat/completions"))
        .json(&json!({
            "stream": false,
            "messages": [
                {
                    "role": "system",
                    "content": request.system_prompt,
                },
                {
                    "role": "user",
                    "content": [
                        {
                            "type": "text",
                            "text": request.user_prompt,
                        },
                        {
                            "type": "image_url",
                            "image_url": {
                                "url": request.image_url,
                            }
                        }
                    ]
                }
            ]
        }))
        .send()
        .await
        .map_err(|error| error.to_string())?;

    let status = response.status();
    let body = response.text().await.map_err(|error| error.to_string())?;
    if !status.is_success() {
        return Err(format!("HTTP {status}: {body}"));
    }

    let value: serde_json::Value =
        serde_json::from_str(&body).map_err(|error| format!("invalid JSON response: {error}"))?;
    value["choices"][0]["message"]["content"]
        .as_str()
        .map(str::trim)
        .filter(|content| !content.is_empty())
        .map(ToOwned::to_owned)
        .ok_or_else(|| "response did not include assistant content".to_string())
}

fn analysis_error(
    screenshot_id: &str,
    screenshot: &ObservationScreenshotCapture,
    app_name: &str,
    window_title: Option<String>,
    message: impl Into<String>,
) -> ActivityCaptureObservationAnalysisError {
    ActivityCaptureObservationAnalysisError {
        observation_id: screenshot.observation_id.clone(),
        screenshot_id: screenshot_id.to_string(),
        screenshot_kind: screenshot.kind.as_str().to_string(),
        captured_at_ms: screenshot.captured_at_ms,
        app_name: app_name.to_string(),
        window_title,
        message: message.into(),
    }
}

fn analysis_identity(screenshot: &ObservationScreenshotCapture) -> (String, Option<String>) {
    match &screenshot.image.subject {
        CaptureSubject::Window(window) => (
            window.app_name.clone(),
            (!window.title.is_empty()).then(|| window.title.clone()),
        ),
        CaptureSubject::Display(_) => (
            screenshot.target.app_name.clone(),
            screenshot.target.title.clone(),
        ),
    }
}

fn file_url(path: &Path) -> Result<String, String> {
    Url::from_file_path(path)
        .map(|url| url.to_string())
        .map_err(|_| {
            format!(
                "failed to convert screenshot path to file URL: {}",
                path.display()
            )
        })
}

struct TempScreenshotFile {
    path: PathBuf,
}

impl TempScreenshotFile {
    fn create(screenshot: &ObservationScreenshotCapture) -> Result<Self, String> {
        let path = std::env::temp_dir().join(format!(
            "activity-observation-{}-{}-{}.{}",
            std::process::id(),
            screenshot.captured_at_ms,
            unix_ms_now(),
            extension_for_mime(&screenshot.image.mime_type)
        ));
        std::fs::write(&path, &screenshot.image.image_bytes)
            .map_err(|error| format!("failed to write screenshot image: {error}"))?;
        Ok(Self { path })
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempScreenshotFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

fn extension_for_mime(mime_type: &str) -> &'static str {
    match mime_type {
        "image/jpeg" => "jpg",
        "image/webp" => "webp",
        _ => "png",
    }
}

use std::path::{Path, PathBuf};

use hypr_activity_capture::ActivityScreenshotCapture;
use hypr_screen_core::CaptureSubject;
use reqwest::Client;
use serde_json::json;
use tauri_plugin_local_llm::LocalLlmPluginExt;
use url::Url;

use crate::events::{
    ActivityCaptureScreenshotAnalysis, ActivityCaptureScreenshotAnalysisError, TransitionReason,
    unix_ms_now,
};

struct ScreenshotAnalysisRequest {
    system_prompt: String,
    user_prompt: String,
    image_url: String,
    _temp_image: TempScreenshotFile,
}

pub async fn analyze_screenshot<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    screenshot: &ActivityScreenshotCapture,
) -> Result<ActivityCaptureScreenshotAnalysis, ActivityCaptureScreenshotAnalysisError> {
    let (app_name, window_title) = analysis_identity(screenshot);
    let server_url = app
        .server_url()
        .await
        .map_err(|error| {
            analysis_error(
                screenshot,
                &app_name,
                window_title.clone(),
                error.to_string(),
            )
        })?
        .ok_or_else(|| {
            analysis_error(
                screenshot,
                &app_name,
                window_title.clone(),
                "local-llm server is unavailable".to_string(),
            )
        })?;

    let request = build_request(screenshot, &app_name, window_title.clone())
        .map_err(|error| analysis_error(screenshot, &app_name, window_title.clone(), error))?;

    let summary = call_local_llm(&server_url, &request)
        .await
        .map_err(|error| analysis_error(screenshot, &app_name, window_title.clone(), error))?;

    Ok(ActivityCaptureScreenshotAnalysis {
        fingerprint: screenshot.fingerprint.clone(),
        reason: screenshot.reason,
        captured_at_ms: screenshot.captured_at_ms,
        app_name,
        window_title,
        summary,
    })
}

fn build_request(
    screenshot: &ActivityScreenshotCapture,
    app_name: &str,
    window_title: Option<String>,
) -> Result<ScreenshotAnalysisRequest, String> {
    let system_prompt =
        hypr_template_app::render(hypr_template_app::Template::ActivityCaptureSystem(
            hypr_template_app::ActivityCaptureSystem { language: None },
        ))
        .map_err(|error| error.to_string())?;

    let reason: TransitionReason = screenshot.reason;
    let user_prompt = hypr_template_app::render(hypr_template_app::Template::ActivityCaptureUser(
        Box::new(hypr_template_app::ActivityCaptureUser {
            app_name: app_name.to_string(),
            window_title,
            reason: reason.as_str().to_string(),
            fingerprint: screenshot.fingerprint.clone(),
        }),
    ))
    .map_err(|error| error.to_string())?;

    let temp_image = TempScreenshotFile::create(screenshot)?;
    let image_url = file_url(temp_image.path())?;

    Ok(ScreenshotAnalysisRequest {
        system_prompt,
        user_prompt,
        image_url,
        _temp_image: temp_image,
    })
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

    parse_response_text(&body)
}

fn parse_response_text(body: &str) -> Result<String, String> {
    let value: serde_json::Value =
        serde_json::from_str(body).map_err(|error| format!("invalid JSON response: {error}"))?;
    value["choices"][0]["message"]["content"]
        .as_str()
        .map(str::trim)
        .filter(|content| !content.is_empty())
        .map(ToOwned::to_owned)
        .ok_or_else(|| "response did not include assistant content".to_string())
}

fn analysis_error(
    screenshot: &ActivityScreenshotCapture,
    app_name: &str,
    window_title: Option<String>,
    message: impl Into<String>,
) -> ActivityCaptureScreenshotAnalysisError {
    ActivityCaptureScreenshotAnalysisError {
        fingerprint: screenshot.fingerprint.clone(),
        captured_at_ms: screenshot.captured_at_ms,
        app_name: app_name.to_string(),
        window_title,
        message: message.into(),
    }
}

fn analysis_identity(screenshot: &ActivityScreenshotCapture) -> (String, Option<String>) {
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
    fn create(screenshot: &ActivityScreenshotCapture) -> Result<Self, String> {
        let path = std::env::temp_dir().join(format!(
            "activity-capture-{}-{}-{}.{}",
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_keeps_temp_image_alive_until_drop() {
        let path = std::env::temp_dir().join(format!(
            "activity-capture-test-{}-{}.png",
            std::process::id(),
            unix_ms_now()
        ));
        std::fs::write(&path, [1, 2, 3]).expect("temp file should be written");
        let temp_image = TempScreenshotFile { path };

        let request = ScreenshotAnalysisRequest {
            system_prompt: "system".to_string(),
            user_prompt: "user".to_string(),
            image_url: file_url(temp_image.path()).expect("file URL should be created"),
            _temp_image: temp_image,
        };
        let path = request._temp_image.path().to_path_buf();

        assert!(path.exists());

        drop(request);

        assert!(!path.exists());
    }
}

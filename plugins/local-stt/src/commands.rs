use std::collections::HashMap;

use crate::{
    LocalModel, LocalSttPluginExt, SUPPORTED_MODELS, ServerInfo, SttModelInfo, server::ServerType,
    stt_model_info,
};

#[tauri::command]
#[specta::specta]
pub async fn models_dir<R: tauri::Runtime>(app: tauri::AppHandle<R>) -> Result<String, String> {
    Ok(app.local_stt().models_dir().to_string_lossy().to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn soniqo_model_dir<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    model: LocalModel,
) -> Result<String, String> {
    Ok(app
        .local_stt()
        .soniqo_model_dir(&model)
        .await
        .map_err(|e| e.to_string())?
        .to_string_lossy()
        .to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn list_supported_models() -> Result<Vec<SttModelInfo>, String> {
    Ok(SUPPORTED_MODELS
        .iter()
        .filter(|m| m.is_available_on_current_platform())
        // Whisper models are only usable when the whisper.cpp engine is
        // compiled into this build.
        .filter(|m| cfg!(feature = "whisper-cpp") || !matches!(m, LocalModel::Whisper(_)))
        // Same rule for the Parakeet ONNX engine.
        .filter(|m| cfg!(feature = "parakeet-onnx") || !matches!(m, LocalModel::ParakeetOnnx(_)))
        // Same rule for the Voxtral (llama.cpp) engine.
        .filter(|m| cfg!(feature = "voxtral-llama") || !matches!(m, LocalModel::VoxtralLlama(_)))
        .map(stt_model_info)
        .collect())
}

#[tauri::command]
#[specta::specta]
pub async fn is_model_downloaded<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    model: LocalModel,
) -> Result<bool, String> {
    app.local_stt()
        .is_model_downloaded(&model)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn is_model_downloading<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    model: LocalModel,
) -> Result<bool, String> {
    app.local_stt()
        .is_model_downloading(&model)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn download_model<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    model: LocalModel,
) -> Result<(), String> {
    app.local_stt()
        .download_model(model)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn cancel_download<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    model: LocalModel,
) -> Result<bool, String> {
    app.local_stt()
        .cancel_download(model)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn delete_model<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    model: LocalModel,
) -> Result<(), String> {
    app.local_stt()
        .delete_model(&model)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn start_server<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    model: LocalModel,
) -> Result<String, String> {
    app.local_stt()
        .start_server(model)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn stop_server<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    server_type: Option<ServerType>,
) -> Result<bool, String> {
    app.local_stt()
        .stop_server(server_type)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn get_server_for_model<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    model: LocalModel,
) -> Result<Option<ServerInfo>, String> {
    app.local_stt()
        .get_server_for_model(&model)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn get_servers<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<HashMap<ServerType, ServerInfo>, String> {
    app.local_stt()
        .get_servers()
        .await
        .map_err(|e| e.to_string())
}

const COMMANDS: &[&str] = &[
    "models_dir",
    "list_supported_models",
    "is_server_running",
    "is_model_downloaded",
    "is_model_downloading",
    "download_model",
    "start_server",
    "stop_server",
    "restart_server",
    "get_current_model",
    "set_current_model",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}

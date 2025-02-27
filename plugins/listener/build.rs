const COMMANDS: &[&str] = &[
    "open_microphone_access_settings",
    "open_system_audio_access_settings",
    "get_timeline",
    "subscribe",
    "start_session",
    "stop_session",
];
fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}

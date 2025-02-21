const COMMANDS: &[&str] = &["get_timeline", "start_session", "stop_session"];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}

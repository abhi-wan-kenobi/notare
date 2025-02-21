const COMMANDS: &[&str] = &["load_model", "unload_model", "start_server", "stop_server"];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}

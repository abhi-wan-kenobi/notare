const COMMANDS: &[&str] = &["get_api_base"];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}

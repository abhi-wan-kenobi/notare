const COMMANDS: &[&str] = &["get-api-base"];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}

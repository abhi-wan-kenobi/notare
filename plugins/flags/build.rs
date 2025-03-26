const COMMANDS: &[&str] = &["is_enabled", "enable", "disable"];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}

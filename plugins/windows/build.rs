const COMMANDS: &[&str] = &["show_window"];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}

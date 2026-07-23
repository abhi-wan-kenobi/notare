const COMMANDS: &[&str] = &["export", "export_text", "export_action_items"];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}

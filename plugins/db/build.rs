const COMMANDS: &[&str] = &["execute", "subscribe", "unsubscribe"];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}

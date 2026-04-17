const COMMANDS: &[&str] = &[
    "register_hotkey",
    "unregister_hotkey",
    "check_permissions",
    "request_accessibility_permission",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}

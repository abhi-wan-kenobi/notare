const COMMANDS: &[&str] = &[
    "register_hotkey",
    "unregister_hotkey",
    "register_global_hotkey",
    "unregister_global_hotkey",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}

const COMMANDS: &[&str] = &[
    "get_settings",
    "set_settings",
    "regenerate_token",
    "get_status",
    "get_bridge_path",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}

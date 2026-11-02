const COMMANDS: &[&str] = &[
    "get_settings",
    "set_settings",
    "regenerate_token",
    "get_status",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}

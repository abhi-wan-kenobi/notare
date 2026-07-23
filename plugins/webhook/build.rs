const COMMANDS: &[&str] = &[
    "get_settings",
    "set_settings",
    "set_secret",
    "clear_secret",
    "recent_deliveries",
    "send_webhook",
    "test_webhook",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}

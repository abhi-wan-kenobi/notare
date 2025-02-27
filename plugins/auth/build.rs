const COMMANDS: &[&str] = &[
    "start_oauth_server",
    "cancel_oauth_server",
    "get_from_store",
    "get_from_vault",
    "reset_vault",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}

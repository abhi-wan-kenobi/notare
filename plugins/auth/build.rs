const COMMANDS: &[&str] = &[
    "start_oauth_server",
    "stop_oauth_server",
    "reset_vault",
    "get_from_store",
    "get_from_vault",
    "set_in_store",
    "set_in_vault",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}

const COMMANDS: &[&str] = &[
    "status",
    "start",
    "complete",
    "cancel",
    "logout",
    "access_token",
    "catalog",
    "openrouter_sign_in",
    "openrouter_cancel",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}

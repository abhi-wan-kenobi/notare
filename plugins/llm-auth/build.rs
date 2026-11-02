const COMMANDS: &[&str] = &[
    "status",
    "start",
    "complete",
    "cancel",
    "logout",
    "access_token",
    "catalog",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}

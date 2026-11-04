const COMMANDS: &[&str] = &[
    "start_callback_server",
    "stop_callback_server",
    "take_pending_pair",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}

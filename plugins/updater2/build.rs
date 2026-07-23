const COMMANDS: &[&str] = &[
    "check",
    "download",
    "install",
    "is_downloaded",
    "postinstall",
    "maybe_emit_updated",
    "can_self_update",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}

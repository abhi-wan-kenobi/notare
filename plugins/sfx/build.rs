const COMMANDS: &[&str] = &["play", "stop"];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}

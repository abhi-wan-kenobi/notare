const COMMANDS: &[&str] = &["window_show", "window_set_floating"];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}

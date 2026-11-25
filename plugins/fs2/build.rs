const COMMANDS: &[&str] = &["read_text_file", "remove", "write_text_file"];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}

const COMMANDS: &[&str] = &["opinionated_md_to_html", "list_template_names"];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}

const COMMANDS: &[&str] = &[
    "show",
    "hide",
    "set_phase",
    "update_amplitude",
    "show_orb",
    "hide_orb",
    "start_dictation",
    "stop_dictation",
    "is_dictating",
    "type_text",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}

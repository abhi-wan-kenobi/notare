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
    "deliver_text",
    "clean_text",
    "pause_media",
    "resume_media",
    "set_warm_mic",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}

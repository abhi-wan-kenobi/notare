const COMMANDS: &[&str] = &[
    "get_git_hash",
    "get_fingerprint",
    "get_device_info",
    "opinionated_md_to_html",
    "delete_session_folder",
    "audio_open",
    "audio_exist",
    "audio_delete",
    "audio_path",
    "audio_import",
    "reveal_session_in_finder",
];

fn main() {
    // vergen-gix 10 renamed the entry point: GixBuilder::default() -> Gix::builder()
    // and build() no longer returns a Result. sha(false) still means "emit the
    // full (non-short) VERGEN_GIT_SHA" — the only var this plugin reads (ext.rs).
    let gitcl = vergen_gix::Gix::builder().sha(false).build();
    vergen_gix::Emitter::default()
        .add_instructions(&gitcl)
        .unwrap()
        .emit()
        .unwrap();

    tauri_plugin::Builder::new(COMMANDS).build();
}

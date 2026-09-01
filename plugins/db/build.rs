// Single source of truth for the `sync_platform` cfg — see the file itself
// for why this is `include!`d rather than duplicated (docs/internal/sync-p2p.md §22).
include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../build-support/sync_app_gate.rs"
));

const COMMANDS: &[&str] = &[
    "execute",
    "execute_proxy",
    "execute_transaction",
    "get_meeting",
    "get_meeting_transcript",
    "get_recurring_meeting_history",
    "get_legacy_cleanup_status",
    "get_legacy_import_report",
    "list_meetings",
    "cleanup_legacy_files",
    "run_legacy_import",
    "subscribe",
    "unsubscribe",
    "sync_status",
    "sync_trigger",
    "sync_list_peers",
    "sync_this_device",
    "sync_add_peer",
    "sync_remove_peer",
    "sync_start",
    "sync_stop",
];

fn main() {
    println!(
        "cargo:rerun-if-changed={}/../../build-support/sync_app_gate.rs",
        env!("CARGO_MANIFEST_DIR")
    );
    emit_sync_app_gate_cfg();
    tauri_plugin::Builder::new(COMMANDS).build();
}

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
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}

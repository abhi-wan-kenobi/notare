const COMMANDS: &[&str] = &[
    "available_providers",
    "is_provider_enabled",
    "list_connection_ids",
    "list_calendars",
    "list_events",
    "open_calendar",
    "create_event",
    "parse_meeting_link",
    "google_account_status",
    "google_import_client_json",
    "google_import_client_file",
    "google_connect",
    "google_disconnect",
    "google_reset",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}

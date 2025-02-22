const COMMANDS: &[&str] = &[
    "start_worker",
    "stop_worker",
    "calendar_access_status",
    "contacts_access_status",
    "request_calendar_access",
    "request_contacts_access",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}

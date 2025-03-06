const COMMANDS: &[&str] = &[
    // calendar
    "list_calendars",
    "upsert_calendar",
    // session
    "visit_session",
    "upsert_session",
    "list_sessions",
    "delete_session",
    "get_session",
    "set_session_event",
    "session_add_participant",
    "session_remove_participant",
    "session_list_participants",
    // template
    "list_templates",
    "upsert_template",
    "delete_template",
    // event
    "list_events",
    // config
    "get_config",
    "set_config",
    // user
    "get_self_human",
    "upsert_human",
    "get_self_organization",
    "upsert_organization",
    // chat
    "list_chat_groups",
    "list_chat_messages",
    "create_chat_group",
    "upsert_chat_message",
    // tag
    "list_all_tags",
    "list_session_tags",
    "assign_tag_to_session",
    "unassign_tag_from_session",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}

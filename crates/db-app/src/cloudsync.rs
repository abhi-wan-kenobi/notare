use std::sync::LazyLock;

use hypr_db_core::CloudsyncTableSpec;

/// SYNC-6: the tables actually enabled for CRDT sync. Enabling a table here
/// mutates its data semantics across every paired device, so each table must
/// have a convergence proof (§17-style) before it is added. SYNC-6 part A
/// proved convergence for the real `sessions` + `session_documents` schema
/// (TEXT-PK, STRICT, `deleted_at` tombstones, no resurrect — no FK, per the
/// §19 correction). The table-proofs lane (docs/internal/sync-p2p.md §23)
/// added the same proof for `transcripts` + `action_items` (incl. a
/// realistic-size `words_json` check and the action_items_v2 ALTER TABLE
/// columns) and for `tags` + `session_tags` (incl. the join-table concurrent
/// add/remove case).
const SYNCED_TABLES: &[&str] = &[
    "sessions",
    "session_documents",
    "transcripts",
    "action_items",
    "tags",
    "session_tags",
];

static CLOUDSYNC_TABLE_REGISTRY: LazyLock<Vec<CloudsyncTableSpec>> = LazyLock::new(|| {
    [
        "action_items",
        "calendars",
        "chat_groups",
        "chat_messages",
        "daily_notes",
        "entity_mentions",
        "events",
        "humans",
        "organizations",
        "session_attachments",
        "session_documents",
        "session_participants",
        "session_tags",
        "sessions",
        "tags",
        "templates",
        "transcripts",
    ]
    .into_iter()
    .map(|table_name| CloudsyncTableSpec {
        enabled: SYNCED_TABLES.contains(&table_name),
        table_name: table_name.to_string(),
        crdt_algo: None,
        force_init: None,
    })
    .collect()
});

pub fn cloudsync_table_registry() -> &'static [CloudsyncTableSpec] {
    CLOUDSYNC_TABLE_REGISTRY.as_slice()
}

pub fn cloudsync_alter_guard_required(table_name: &str) -> bool {
    cloudsync_table_registry()
        .iter()
        .any(|table| table.enabled && table.table_name == table_name)
}

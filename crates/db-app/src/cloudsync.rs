use std::sync::LazyLock;

use hypr_db_core::CloudsyncTableSpec;

/// SYNC-5: the tables actually enabled for CRDT sync. Enabling a table here
/// mutates its data semantics across every paired device, so this list must
/// only ever contain tables a convergence proof has covered.
///
/// **It is empty on purpose.** The SYNC-4 proofs
/// (`crates/sync-p2p/examples/sync_three_nodes.rs`, `sync_two_nodes.rs`,
/// `drain_regression.rs`) converged a synthetic `notes (id INTEGER PRIMARY
/// KEY, body TEXT)` table — not one notare app table. The transport
/// (iroh, hub, drain, token) is proven; no app table's row shape has been
/// through a convergence proof (sessions carries tombstone `deleted_at`,
/// STRICT typing and JSON columns whose CRDT behavior is unproven). SYNC-6
/// extends this list table-by-table, proof first.
const SYNCED_TABLES: &[&str] = &[];

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

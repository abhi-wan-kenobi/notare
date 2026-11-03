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
/// add/remove case). Those proofs were measured against sqlite-sync 1.0.12;
/// under cr-sqlite these are the tables `crsql_as_crr` converts when sync is
/// turned on, and nothing new is enabled until it is re-proved there.
///
/// §25 deliberately did **not** add `humans` or `session_participants`,
/// which are proven *not* safe to enable rather than merely unproven: both
/// are written with a locally-minted random-UUID id behind a
/// `NOT EXISTS (...)` guard that is evaluated locally, so two offline
/// devices adding the same person (or the same person to the same session)
/// each pass their own guard, mint different primary keys, and keep both
/// rows forever after the merge. No merge engine can unify two distinct
/// primary keys. Do not add either without first making their ids
/// deterministic.
///
/// §26 found the same duplication defect in `calendars` and `events`, which
/// are worse: the duplicating write is the calendar poller, so it fires on
/// every device automatically. Both keep the provider's real identity in a
/// non-PK column (`tracking_id_calendar` / `tracking_id_event`) while the
/// PK is a local random UUID, and both carry per-device connection state.
/// NO-GO for both.
///
/// `organizations` (find-or-create-by-name behind a local guard, §25.3
/// correction), `templates` (seeder resurrection, §27.4), `chat_groups` and
/// `chat_messages` (§29) are eligible pending a cr-sqlite proof — see
/// [`ELIGIBLE_PENDING_PROOF`].
pub const SYNCED_TABLES: &[&str] = &[
    "sessions",
    "session_documents",
    "transcripts",
    "action_items",
    "tags",
    "session_tags",
];

/// Tables *eligible* for sync once their cr-sqlite proofs land, but **not**
/// enabled. Lets the pinning test assert what a bare absence from
/// [`SYNCED_TABLES`] cannot: these are known-eligible rather than NO-GO,
/// and none was enabled before its proof.
pub const ELIGIBLE_PENDING_PROOF: &[&str] =
    &["organizations", "templates", "chat_groups", "chat_messages"];

/// Proven unsafe to sync (§25.2, §26.2): each duplicates its entity under
/// concurrent creation. Must never appear in [`SYNCED_TABLES`].
pub const NO_GO_TABLES: &[&str] = &["humans", "session_participants", "calendars", "events"];

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

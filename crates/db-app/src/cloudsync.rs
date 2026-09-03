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
///
/// §25's contacts batch added `organizations`. It deliberately did **not**
/// add `humans` or `session_participants`, which are proven *not* safe to
/// enable rather than merely unproven: both are written with a
/// `crypto.randomUUID()` id behind a `NOT EXISTS (...)` guard that is
/// evaluated locally, so two offline devices adding the same person (or the
/// same person to the same session) each pass their own guard, mint
/// different primary keys, and keep both rows forever after the merge. The
/// rows converge; the entity is duplicated, and CLS cannot merge two
/// distinct primary keys. Do not add either without first making their ids
/// deterministic — `crates/sync-p2p/examples/sync_contacts_schema.rs`
/// scenarios 3 and 5 fail loudly if that changes.
///
/// §26 found the same duplication defect in `calendars` and `events`, which
/// are worse: the duplicating write is the calendar poller, not a user
/// action, so it fires on every device automatically. Both keep the
/// provider's real identity in a non-PK column (`tracking_id_calendar` /
/// `tracking_id_event`) while the PK is a local
/// `crypto.randomUUID()`. NO-GO for both.
///
/// §27 added `templates`, which is safe for the opposite reason: its
/// built-in rows are seeded with fixed content-derived ids
/// (`default-daily-standup`, ...), so independent seeding on two devices
/// converges to one row instead of forking. It is also the first table here
/// with a real hard `DELETE` (`template_ops.rs:75`; the table has no
/// `deleted_at`), which §23.7 had named as an untested case — it
/// propagates without resurrection. One caveat recorded in §27.4: a fresh
/// device's migration re-seed resurrects a default template deleted
/// elsewhere. That is a seeder bug, not a CRDT one, but enabling this table
/// makes it cross-device.
///
/// §29 added `chat_groups` + `chat_messages`, the last two live tables.
/// Both use `ON CONFLICT(id) DO UPDATE` with an id minted once on the
/// creating device and no dedup guard, so they do not fork. Concurrent
/// appends to one conversation both survive — two different messages
/// correctly stay two messages — and both nodes agree on the rendered
/// transcript order. One caveat in §29.3: `deleteChatMessagesExcept`'s
/// retained set is computed from the writing device's local view, so a
/// regenerate cannot prune a message it has never seen. App logic, not
/// convergence.
const SYNCED_TABLES: &[&str] = &[
    "sessions",
    "session_documents",
    "transcripts",
    "action_items",
    "tags",
    "session_tags",
    "organizations",
    "templates",
    "chat_groups",
    "chat_messages",
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

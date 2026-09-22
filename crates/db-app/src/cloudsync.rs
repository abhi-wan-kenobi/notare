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
/// the engine is being swapped to cr-sqlite, so every enabled table must be
/// re-proved under it (§31 proof matrix) and nothing new is enabled until
/// then.
///
/// §25 deliberately did **not** add `humans` or `session_participants`,
/// which are proven *not* safe to enable rather than merely unproven: both
/// are written with a locally-minted random-UUID id behind a
/// `NOT EXISTS (...)` guard that is evaluated locally, so two offline
/// devices adding the same person (or the same person to the same session)
/// each pass their own guard, mint different primary keys, and keep both
/// rows forever after the merge. The rows converge; the entity is
/// duplicated, and no merge engine can unify two distinct primary keys. Do
/// not add either without first making their ids deterministic (the §25
/// fix, PR slice B4).
///
/// §26 found the same duplication defect in `calendars` and `events`, which
/// are worse: the duplicating write is the calendar poller, not a user
/// action, so it fires on every device automatically. Both keep the
/// provider's real identity in a non-PK column (`tracking_id_calendar` /
/// `tracking_id_event`) while the PK is a local random UUID. NO-GO for
/// both. A further hazard: they carry per-device connection state
/// (`calendars.enabled`, poller tombstones on disconnect), so syncing them
/// would tombstone events everywhere when one device disconnects — the
/// deterministic-id fix (B5) makes ids portable but both stay disabled.
///
/// `organizations` was judged GO in §25, but on a false premise (§25.3
/// claimed no find-or-create-by-name path exists; `updateHuman` with
/// `companyName` does mint an id behind a `NOT EXISTS lower(name)` guard —
/// same defect class as `humans`, milder consequence). It stays
/// eligible-pending-proof rather than enabled, and whether it joins the
/// deterministic-id pass is a product call.
///
/// `templates` (§27) is eligible for the opposite reason: its built-in rows
/// are seeded with fixed content-derived ids (`default-daily-standup`, …),
/// so independent seeding on two devices converges to one row instead of
/// forking. Its open item is the seeder, not the engine: a fresh device's
/// migration re-seed can resurrect a default template deleted elsewhere
/// (§27.4) — fixed by the seed-once marker + `deleted_at` tombstone (B3)
/// and then proven as engine row T-5.
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
];

/// Tables that are *eligible* for sync once their cr-sqlite proofs land
/// (§31 proof matrix, batch 2), but are **not** enabled today. This list
/// exists so the pinning test can assert two things a bare absence from
/// `SYNCED_TABLES` cannot: these tables are known-eligible rather than
/// NO-GO, and nothing here has been enabled before its proof. When a
/// batch's proofs pass, its tables move from here into `SYNCED_TABLES`
/// (PR slice B8 is the only PR allowed to touch either list).
pub(crate) const ELIGIBLE_PENDING_PROOF: &[&str] = &[
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

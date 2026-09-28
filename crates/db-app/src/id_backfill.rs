//! Deterministic id backfill for `humans`, `session_participants`, and
//! `organizations` (sync plan 2026-09-07, workstream B4).
//!
//! Ids are `prefix + hex(sha256(domain-scoped input))[0..32]` so that two
//! devices independently minting a row for the same email/name/participant
//! converge on one row instead of colliding UUIDs:
//!
//! ```text
//! humans.id               = "h_"  + hex(sha256("notare:human:v1:"       + norm_email))[0..32]
//! organizations.id        = "o_"  + hex(sha256("notare:org:v1:"         + norm_name))[0..32]
//! session_participants.id = "sp_" + hex(sha256("notare:participant:v1:" + session_id + "\n" + member_key))[0..32]
//! ```
//!
//! Rules (mirrored byte-for-byte by `apps/desktop/src/shared/ids.ts`):
//! - `norm_email = lower(trim(email))` after NFC; `norm_name = lower(trim(name))`.
//! - Empty email (or empty name for orgs) keeps the existing random UUID.
//! - `id = owner_user_id` humans (the self-human) are never rewritten.
//! - `member_key` is `"h:" + human_id` when `human_id` is non-empty, else
//!   `"e:" + norm_email`, else the row keeps its UUID.
//! - Identity is fixed at creation: editing an email or name later must not
//!   change the id, and every minting site keeps its `NOT EXISTS` guard.
//!
//! The backfill is idempotent: it runs once, guarded by the `app_settings` row
//! `id_scheme_v1_applied` (`app_settings` is not in the sync registry, so the
//! marker stays local to this device), and rewrites every reference to a
//! rewritten id in the same transaction. In-database duplicates by normalized
//! email/name/(session, member) collapse to the oldest row: the survivor is
//! renamed to the deterministic id and the duplicates are tombstoned, with
//! every reference redirected to the survivor.

use sha2::{Digest, Sha256};
use sqlx::SqlitePool;
use unicode_normalization::UnicodeNormalization;

pub const ID_SCHEME_MARKER: &str = "id_scheme_v1_applied";

const HUMAN_DOMAIN: &str = "notare:human:v1:";
const ORG_DOMAIN: &str = "notare:org:v1:";
const PARTICIPANT_DOMAIN: &str = "notare:participant:v1:";

/// Deterministic `humans.id` for an email. Empty email returns `None`
/// (callers keep a random UUID).
pub fn human_id_for_email(email: &str) -> Option<String> {
    let normalized = normalize_email(email);
    if normalized.is_empty() {
        return None;
    }
    Some(format!(
        "h_{}",
        &hex_digest(&[HUMAN_DOMAIN, &normalized])[..32]
    ))
}

/// Deterministic `organizations.id` for a name. Empty name returns `None`.
pub fn organization_id_for_name(name: &str) -> Option<String> {
    let normalized = normalize_name(name);
    if normalized.is_empty() {
        return None;
    }
    Some(format!(
        "o_{}",
        &hex_digest(&[ORG_DOMAIN, &normalized])[..32]
    ))
}

/// Deterministic `session_participants.id` for a (session, member) pair.
/// Rows with neither a human nor an email keep their UUID (`None`).
pub fn participant_id(session_id: &str, human_id: &str, email: &str) -> Option<String> {
    let member_key = member_key_for(human_id, email)?;
    Some(format!(
        "sp_{}",
        &hex_digest(&[PARTICIPANT_DOMAIN, session_id, "\n", &member_key])[..32]
    ))
}

fn member_key_for(human_id: &str, email: &str) -> Option<String> {
    if !human_id.trim().is_empty() {
        return Some(format!("h:{human_id}"));
    }
    let normalized = normalize_email(email);
    if normalized.is_empty() {
        return None;
    }
    Some(format!("e:{normalized}"))
}

/// `lower(trim(value))` after NFC normalization.
pub fn normalize_email(email: &str) -> String {
    email.trim().nfc().collect::<String>().to_lowercase()
}

/// `lower(trim(value))` after NFC normalization (same as [`normalize_email`]).
pub fn normalize_name(name: &str) -> String {
    normalize_email(name)
}

fn hex_digest(parts: &[&str]) -> String {
    let mut hasher = Sha256::new();
    for part in parts {
        hasher.update(part.as_bytes());
    }
    let digest = hasher.finalize();
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Rewrites legacy UUID ids on `humans`, `organizations`, and
/// `session_participants` to their deterministic form and updates every
/// reference, in one transaction. Runs at most once per database, guarded by
/// the `app_settings` marker row; a second call is a no-op.
pub async fn backfill_deterministic_ids(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;

    // Checked again inside the transaction (not just by the caller) so two
    // overlapping calls on the same pool can't both decide to run: SQLite
    // serializes writers, so the loser here sees the marker the winner just
    // committed rather than repeating the whole rewrite.
    let applied: Option<String> =
        sqlx::query_scalar("SELECT value_json FROM app_settings WHERE id = ?")
            .bind(ID_SCHEME_MARKER)
            .fetch_optional(&mut *tx)
            .await?;
    if applied.is_some() {
        return tx.rollback().await;
    }

    let human_rewrites = backfill_humans(&mut tx).await?;
    apply_human_id_renames(&mut tx, &human_rewrites).await?;
    apply_human_reference_rewrites(&mut tx, &human_rewrites).await?;

    backfill_organizations(&mut tx).await?;
    backfill_session_participants(&mut tx).await?;

    sqlx::query("INSERT OR IGNORE INTO app_settings (id, value_json) VALUES (?, 'true')")
        .bind(ID_SCHEME_MARKER)
        .execute(&mut *tx)
        .await?;

    tx.commit().await
}

type Tx<'c> = sqlx::Transaction<'c, sqlx::Sqlite>;

/// Old human id -> deterministic id. `renames` covers rows whose own id
/// changes (survivors only); `redirects` additionally covers duplicate rows
/// that collapse into a survivor, so referencing columns can follow the
/// move. A duplicate is never renamed (that would collide with the
/// survivor's new primary key); it is tombstoned and left on its old id.
struct HumanRewrites {
    renames: std::collections::HashMap<String, String>,
    redirects: std::collections::HashMap<String, String>,
}

struct Candidate {
    id: String,
    deleted_at: Option<String>,
}

/// Picks which row in a normalized-key group survives a collapse, and
/// whether the survivor needs undeleting: a row already sitting on `new_id`
/// (e.g. synced in from a device that already backfilled) must win even if
/// it is older or soft-deleted — renaming another row onto that id would
/// violate the primary key. Otherwise the oldest non-deleted row wins, so an
/// active duplicate never gets redirected to an already-tombstoned survivor;
/// only when every row in the group is deleted does the oldest deleted row
/// win. The group is undeleted if any member (survivor or not) was active,
/// since it represents one logical identity going forward.
fn pick_survivor(new_id: &str, candidates: &[Candidate]) -> (usize, bool) {
    let any_active = candidates.iter().any(|c| c.deleted_at.is_none());
    let idx = candidates
        .iter()
        .position(|c| c.id == new_id)
        .or_else(|| candidates.iter().position(|c| c.deleted_at.is_none()))
        .unwrap_or(0);
    (idx, any_active)
}

async fn backfill_humans(tx: &mut Tx<'_>) -> Result<HumanRewrites, sqlx::Error> {
    use sqlx::Row;

    // Skip the self-human (id = owner_user_id; converges by construction)
    // and rows with no email (keep the UUID). Duplicates by normalized email
    // collapse onto one survivor (see `pick_survivor`); the others are
    // tombstoned and their references redirected to the survivor.
    let rows = sqlx::query(
        "SELECT id, owner_user_id, email, deleted_at
         FROM humans
         ORDER BY created_at, id",
    )
    .fetch_all(&mut **tx)
    .await?;

    let mut groups: std::collections::HashMap<String, Vec<Candidate>> =
        std::collections::HashMap::new();

    for row in rows {
        let old_id: String = row.get("id");
        let owner_user_id: String = row.get("owner_user_id");
        let email: String = row.get("email");
        let deleted_at: Option<String> = row.get("deleted_at");

        if !owner_user_id.is_empty() && old_id == owner_user_id {
            continue;
        }

        let Some(new_id) = human_id_for_email(&email) else {
            continue;
        };

        groups.entry(new_id).or_default().push(Candidate {
            id: old_id,
            deleted_at,
        });
    }

    let mut rewrites = HumanRewrites {
        renames: std::collections::HashMap::new(),
        redirects: std::collections::HashMap::new(),
    };

    for (new_id, candidates) in groups {
        let (survivor_idx, any_active) = pick_survivor(&new_id, &candidates);

        for (i, candidate) in candidates.iter().enumerate() {
            rewrites
                .redirects
                .insert(candidate.id.clone(), new_id.clone());

            if i == survivor_idx {
                if any_active && candidate.deleted_at.is_some() {
                    sqlx::query(
                        "UPDATE humans
                         SET deleted_at = NULL,
                             updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                         WHERE id = ?",
                    )
                    .bind(&candidate.id)
                    .execute(&mut **tx)
                    .await?;
                }
                if candidate.id != new_id {
                    rewrites
                        .renames
                        .insert(candidate.id.clone(), new_id.clone());
                }
            } else if candidate.deleted_at.is_none() {
                sqlx::query(
                    "UPDATE humans
                     SET deleted_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                         updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                     WHERE id = ?",
                )
                .bind(&candidate.id)
                .execute(&mut **tx)
                .await?;
            }
        }
    }

    Ok(rewrites)
}

async fn apply_human_id_renames(
    tx: &mut Tx<'_>,
    rewrites: &HumanRewrites,
) -> Result<(), sqlx::Error> {
    for (old_id, new_id) in &rewrites.renames {
        sqlx::query("UPDATE humans SET id = ? WHERE id = ?")
            .bind(new_id)
            .bind(old_id)
            .execute(&mut **tx)
            .await?;
    }
    Ok(())
}

/// Rewrites every column that references a human id, including ids embedded
/// in `transcripts.speaker_hints_json`.
async fn apply_human_reference_rewrites(
    tx: &mut Tx<'_>,
    rewrites: &HumanRewrites,
) -> Result<(), sqlx::Error> {
    use sqlx::Row;

    for (old_id, new_id) in &rewrites.redirects {
        sqlx::query("UPDATE session_participants SET human_id = ? WHERE human_id = ?")
            .bind(new_id)
            .bind(old_id)
            .execute(&mut **tx)
            .await?;

        sqlx::query("UPDATE action_items SET assignee_human_id = ? WHERE assignee_human_id = ?")
            .bind(new_id)
            .bind(old_id)
            .execute(&mut **tx)
            .await?;

        sqlx::query("UPDATE voice_profiles SET human_id = ? WHERE human_id = ?")
            .bind(new_id)
            .bind(old_id)
            .execute(&mut **tx)
            .await?;

        sqlx::query(
            "UPDATE entity_mentions SET target_id = ? WHERE target_id = ? AND target_type = 'human'",
        )
        .bind(new_id)
        .bind(old_id)
        .execute(&mut **tx)
        .await?;
    }

    // transcripts.speaker_hints_json embeds human ids inside JSON; rewrite
    // every "human_id" key via serde_json, tolerating unknown shapes.
    if rewrites.redirects.is_empty() {
        return Ok(());
    }
    let rows = sqlx::query("SELECT id, speaker_hints_json FROM transcripts")
        .fetch_all(&mut **tx)
        .await?;
    for row in rows {
        let transcript_id: String = row.get("id");
        let hints_json: String = row.get("speaker_hints_json");
        if !rewrites
            .redirects
            .keys()
            .any(|old| hints_json.contains(old.as_str()))
        {
            continue;
        }
        let Ok(mut hints) = serde_json::from_str::<serde_json::Value>(&hints_json) else {
            continue; // tolerate unknown/legacy shapes; leave them untouched
        };
        if rewrite_human_ids_in_json(&mut hints, &rewrites.redirects) {
            let rewritten =
                serde_json::to_string(&hints).map_err(|error| sqlx::Error::Encode(error.into()))?;
            sqlx::query("UPDATE transcripts SET speaker_hints_json = ? WHERE id = ?")
                .bind(&rewritten)
                .bind(&transcript_id)
                .execute(&mut **tx)
                .await?;
        }
    }

    Ok(())
}

fn rewrite_human_ids_in_json(
    value: &mut serde_json::Value,
    redirects: &std::collections::HashMap<String, String>,
) -> bool {
    let mut changed = false;
    match value {
        serde_json::Value::Object(map) => {
            for (key, entry) in map.iter_mut() {
                if key == "human_id"
                    && let serde_json::Value::String(id) = entry
                    && let Some(new_id) = redirects.get(id)
                {
                    *entry = serde_json::Value::String(new_id.clone());
                    changed = true;
                } else {
                    changed |= rewrite_human_ids_in_json(entry, redirects);
                }
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                changed |= rewrite_human_ids_in_json(item, redirects);
            }
        }
        _ => {}
    }
    changed
}

async fn backfill_organizations(tx: &mut Tx<'_>) -> Result<(), sqlx::Error> {
    use sqlx::Row;

    let rows = sqlx::query(
        "SELECT id, name, deleted_at
         FROM organizations
         ORDER BY created_at, id",
    )
    .fetch_all(&mut **tx)
    .await?;

    let mut groups: std::collections::HashMap<String, Vec<Candidate>> =
        std::collections::HashMap::new();

    for row in rows {
        let old_id: String = row.get("id");
        let name: String = row.get("name");
        let deleted_at: Option<String> = row.get("deleted_at");

        let Some(new_id) = organization_id_for_name(&name) else {
            continue;
        };

        groups.entry(new_id).or_default().push(Candidate {
            id: old_id,
            deleted_at,
        });
    }

    let mut renames: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    let mut redirects: std::collections::HashMap<String, String> = std::collections::HashMap::new();

    for (new_id, candidates) in groups {
        let (survivor_idx, any_active) = pick_survivor(&new_id, &candidates);

        for (i, candidate) in candidates.iter().enumerate() {
            redirects.insert(candidate.id.clone(), new_id.clone());

            if i == survivor_idx {
                if any_active && candidate.deleted_at.is_some() {
                    sqlx::query(
                        "UPDATE organizations
                         SET deleted_at = NULL,
                             updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                         WHERE id = ?",
                    )
                    .bind(&candidate.id)
                    .execute(&mut **tx)
                    .await?;
                }
                if candidate.id != new_id {
                    renames.insert(candidate.id.clone(), new_id.clone());
                }
            } else if candidate.deleted_at.is_none() {
                sqlx::query(
                    "UPDATE organizations
                     SET deleted_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                         updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                     WHERE id = ?",
                )
                .bind(&candidate.id)
                .execute(&mut **tx)
                .await?;
            }
        }
    }

    for (old_id, new_id) in &renames {
        sqlx::query("UPDATE organizations SET id = ? WHERE id = ?")
            .bind(new_id)
            .bind(old_id)
            .execute(&mut **tx)
            .await?;
    }
    for (old_id, new_id) in &redirects {
        sqlx::query("UPDATE humans SET organization_id = ? WHERE organization_id = ?")
            .bind(new_id)
            .bind(old_id)
            .execute(&mut **tx)
            .await?;
    }

    Ok(())
}

async fn backfill_session_participants(tx: &mut Tx<'_>) -> Result<(), sqlx::Error> {
    use sqlx::Row;

    // Runs after the humans rewrite, so human_id is already deterministic.
    // Collisions on (session_id, member_key) collapse onto one survivor (see
    // `pick_survivor`). The duplicate keeps its old (now tombstoned) id:
    // nothing references a participant id from another table.
    let rows = sqlx::query(
        "SELECT id, session_id, human_id, email, deleted_at
         FROM session_participants
         ORDER BY created_at, id",
    )
    .fetch_all(&mut **tx)
    .await?;

    let mut groups: std::collections::HashMap<String, Vec<Candidate>> =
        std::collections::HashMap::new();

    for row in rows {
        let old_id: String = row.get("id");
        let session_id: String = row.get("session_id");
        let human_id: String = row.get("human_id");
        let email: String = row.get("email");
        let deleted_at: Option<String> = row.get("deleted_at");

        let Some(new_id) = participant_id(&session_id, &human_id, &email) else {
            continue;
        };

        groups.entry(new_id).or_default().push(Candidate {
            id: old_id,
            deleted_at,
        });
    }

    let mut renames: std::collections::HashMap<String, String> = std::collections::HashMap::new();

    for (new_id, candidates) in groups {
        let (survivor_idx, any_active) = pick_survivor(&new_id, &candidates);

        for (i, candidate) in candidates.iter().enumerate() {
            if i == survivor_idx {
                if any_active && candidate.deleted_at.is_some() {
                    sqlx::query(
                        "UPDATE session_participants
                         SET deleted_at = NULL,
                             updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                         WHERE id = ?",
                    )
                    .bind(&candidate.id)
                    .execute(&mut **tx)
                    .await?;
                }
                if candidate.id != new_id {
                    renames.insert(candidate.id.clone(), new_id.clone());
                }
            } else if candidate.deleted_at.is_none() {
                sqlx::query(
                    "UPDATE session_participants
                     SET deleted_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                         updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                     WHERE id = ?",
                )
                .bind(&candidate.id)
                .execute(&mut **tx)
                .await?;
            }
        }
    }

    for (old_id, new_id) in &renames {
        sqlx::query("UPDATE session_participants SET id = ? WHERE id = ?")
            .bind(new_id)
            .bind(old_id)
            .execute(&mut **tx)
            .await?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_test_vector_matches_the_ts_implementation() {
        // Byte-identical with apps/desktop/src/shared/ids.test.ts.
        assert_eq!(
            human_id_for_email("alice@Example.com "),
            Some("h_4fa1d59c20dd48d285b666b890067c7f".to_string())
        );
        assert_eq!(
            organization_id_for_name("  Example Inc  "),
            Some("o_b43677833502048ea625456d505a4c33".to_string())
        );
        let human_id = human_id_for_email("alice@example.com").unwrap();
        assert_eq!(
            participant_id("session-1", &human_id, ""),
            Some("sp_668289d5958cac9b021d2b3637a4b4f7".to_string())
        );
        assert_eq!(
            participant_id("session-1", "", "Bob@Example.com"),
            Some("sp_21ea03a4de5b32b54dce588d3e6441ea".to_string())
        );
    }

    #[test]
    fn empty_identity_inputs_keep_random_uuids() {
        assert_eq!(human_id_for_email(""), None);
        assert_eq!(human_id_for_email("   "), None);
        assert_eq!(organization_id_for_name(""), None);
        assert_eq!(participant_id("session-1", "", ""), None);
    }

    #[test]
    fn normalize_email_applies_nfc_so_equivalent_unicode_forms_converge() {
        // Precomposed "é" (U+00E9) vs. "e" + combining acute accent
        // (U+0065 U+0301): same visible string, different bytes. Without NFC
        // these hash to different ids and two devices never converge.
        let composed = "caf\u{00e9}@example.com";
        let decomposed = "cafe\u{0301}@example.com";
        assert_ne!(composed, decomposed);
        assert_eq!(normalize_email(composed), normalize_email(decomposed));
        assert_eq!(human_id_for_email(composed), human_id_for_email(decomposed));
    }

    async fn open_test_db() -> hypr_db_core::Db {
        let db = hypr_db_core::Db::open(hypr_db_core::DbOpenOptions {
            storage: hypr_db_core::DbStorage::Memory,
            cloudsync_enabled: false,
            journal_mode_wal: true,
            foreign_keys: true,
            max_connections: Some(1),
        })
        .await
        .unwrap();
        hypr_db_migrate::migrate(
            &db,
            hypr_db_migrate::DbSchema {
                steps: crate::APP_MIGRATION_STEPS,
                validate_cloudsync_table: crate::cloudsync_alter_guard_required,
            },
        )
        .await
        .unwrap();
        db
    }

    #[tokio::test]
    async fn collapse_prefers_the_active_row_over_an_older_deleted_one() {
        let db = open_test_db().await;
        let pool = db.pool();

        sqlx::query(
            "INSERT INTO humans (id, owner_user_id, email, name, created_at, deleted_at)
             VALUES
               ('uuid-old-deleted', 'user-1', 'alice@example.com', 'Old', '2026-01-01T00:00:00.000Z', '2026-01-02T00:00:00.000Z'),
               ('uuid-new-active', 'user-1', 'alice@example.com', 'New', '2026-02-01T00:00:00.000Z', NULL)",
        )
        .execute(pool)
        .await
        .unwrap();

        backfill_deterministic_ids(pool).await.unwrap();

        let alice_id = human_id_for_email("alice@example.com").unwrap();
        let active: Option<String> = sqlx::query_scalar(
            "SELECT id FROM humans WHERE email = 'alice@example.com' AND deleted_at IS NULL",
        )
        .fetch_optional(pool)
        .await
        .unwrap();
        // The oldest *non-deleted* row survives active, not the older
        // deleted one — a duplicate must never eclipse a live contact.
        assert_eq!(active, Some(alice_id));
    }

    #[tokio::test]
    async fn collapse_never_renames_onto_an_id_another_row_already_holds() {
        let db = open_test_db().await;
        let pool = db.pool();
        let bob_id = human_id_for_email("bob@example.com").unwrap();

        // A legacy UUID row (older) and a row already sitting on the
        // deterministic id (e.g. synced in from a device that already
        // backfilled) both exist for the same email. Naively renaming the
        // older row onto `bob_id` would collide with the primary key.
        sqlx::query(
            "INSERT INTO humans (id, owner_user_id, email, name, created_at, deleted_at)
             VALUES
               ('uuid-legacy', 'user-1', 'bob@example.com', 'Legacy', '2026-01-01T00:00:00.000Z', NULL),
               (?, 'user-1', 'bob@example.com', 'Synced', '2026-02-01T00:00:00.000Z', NULL)",
        )
        .bind(&bob_id)
        .execute(pool)
        .await
        .unwrap();

        backfill_deterministic_ids(pool).await.unwrap();

        let active: Vec<String> = sqlx::query_scalar(
            "SELECT id FROM humans WHERE email = 'bob@example.com' AND deleted_at IS NULL",
        )
        .fetch_all(pool)
        .await
        .unwrap();
        assert_eq!(active, vec![bob_id]);
    }

    #[tokio::test]
    async fn backfill_is_idempotent_and_rewrites_references() {
        use crate::APP_MIGRATION_STEPS;
        use hypr_db_core::Db;

        let db = Db::open(hypr_db_core::DbOpenOptions {
            storage: hypr_db_core::DbStorage::Memory,
            cloudsync_enabled: false,
            journal_mode_wal: true,
            foreign_keys: true,
            max_connections: Some(1),
        })
        .await
        .unwrap();
        hypr_db_migrate::migrate(
            &db,
            hypr_db_migrate::DbSchema {
                steps: APP_MIGRATION_STEPS,
                validate_cloudsync_table: crate::cloudsync_alter_guard_required,
            },
        )
        .await
        .unwrap();
        let pool = db.pool();

        // Legacy fixture: UUID humans (one duplicate email), a self-human,
        // an email-less human, orgs (one duplicate name), participants, and
        // referencing rows.
        sqlx::query(
            "INSERT INTO humans (id, owner_user_id, email, name, created_at)
             VALUES
               ('uuid-alice', 'user-1', 'alice@example.com', 'Alice', '2026-01-01T00:00:00.000Z'),
               ('uuid-alice-2', 'user-1', 'Alice@Example.com ', 'Alice Dup', '2026-02-01T00:00:00.000Z'),
               ('00000000-0000-0000-0000-000000000000', '00000000-0000-0000-0000-000000000000', '', 'Self', '2026-01-01T00:00:00.000Z'),
               ('uuid-nameless', 'user-1', '', 'No Email', '2026-01-01T00:00:00.000Z')",
        )
        .execute(pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO organizations (id, name, created_at)
             VALUES ('uuid-acme', 'Acme Corp', '2026-01-01T00:00:00.000Z'),
                    ('uuid-acme-2', 'acme corp', '2026-03-01T00:00:00.000Z')",
        )
        .execute(pool)
        .await
        .unwrap();
        sqlx::query("UPDATE humans SET organization_id = 'uuid-acme-2' WHERE id = 'uuid-alice'")
            .execute(pool)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO sessions (id, title) VALUES ('session-1', 'Test'), ('session-2', 'Test')",
        )
        .execute(pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO session_participants (id, session_id, human_id, display_name, email, created_at)
             VALUES
               ('uuid-p1', 'session-1', 'uuid-alice', 'Alice', 'alice@example.com', '2026-01-05T00:00:00.000Z'),
               ('uuid-p2', 'session-2', '', 'Bob', 'bob@example.com', '2026-01-06T00:00:00.000Z'),
               ('uuid-p3', 'session-2', '', 'Ghost', '', '2026-01-07T00:00:00.000Z')",
        )
        .execute(pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO action_items (id, session_id, assignee_human_id, text)
             VALUES ('a1', 'session-1', 'uuid-alice-2', 'Do it')",
        )
        .execute(pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO voice_profiles (id, human_id, embedding, dim, model)
             VALUES ('vp1', 'uuid-alice', X'0102', 2, 'test')",
        )
        .execute(pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO entity_mentions (id, source_type, source_id, target_type, target_id)
             VALUES ('em1', 'note', 'doc-1', 'human', 'uuid-alice'),
                    ('em2', 'note', 'doc-1', 'tag', 'uuid-alice')",
        )
        .execute(pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO transcripts (id, session_id, speaker_hints_json)
             VALUES
               ('t1', 'session-1',
                '[{\"human_id\":\"uuid-alice\",\"name\":\"Alice\"},{\"unknown\":7}]'),
               ('t2', 'session-2', 'not json at all')",
        )
        .execute(pool)
        .await
        .unwrap();

        let alice_id = human_id_for_email("alice@example.com").unwrap();
        let acme_id = organization_id_for_name("Acme Corp").unwrap();
        let p1_id = participant_id("session-1", &alice_id, "").unwrap();
        let p2_id = participant_id("session-2", "", "bob@example.com").unwrap();

        backfill_deterministic_ids(pool).await.unwrap();

        // Alice converged to the deterministic id; the duplicate is
        // tombstoned (kept on its old id) and gone from the active set.
        let alice: Option<String> = sqlx::query_scalar(
            "SELECT id FROM humans WHERE email = 'alice@example.com' AND deleted_at IS NULL",
        )
        .fetch_optional(pool)
        .await
        .unwrap();
        assert_eq!(alice, Some(alice_id.clone()));
        let dup_deleted_at: Option<String> =
            sqlx::query_scalar("SELECT deleted_at FROM humans WHERE id = 'uuid-alice-2'")
                .fetch_one(pool)
                .await
                .unwrap();
        assert!(dup_deleted_at.is_some());

        // Self-human and email-less rows untouched.
        let self_row: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM humans WHERE id = '00000000-0000-0000-0000-000000000000'",
        )
        .fetch_one(pool)
        .await
        .unwrap();
        assert_eq!(self_row, 1);
        let nameless: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM humans WHERE id = 'uuid-nameless'")
                .fetch_one(pool)
                .await
                .unwrap();
        assert_eq!(nameless, 1);

        // References rewritten — including from the duplicate's old id.
        let assignee: String =
            sqlx::query_scalar("SELECT assignee_human_id FROM action_items WHERE id = 'a1'")
                .fetch_one(pool)
                .await
                .unwrap();
        assert_eq!(assignee, alice_id);
        let vp: String = sqlx::query_scalar("SELECT human_id FROM voice_profiles WHERE id = 'vp1'")
            .fetch_one(pool)
            .await
            .unwrap();
        assert_eq!(vp, alice_id);
        let em_human: String =
            sqlx::query_scalar("SELECT target_id FROM entity_mentions WHERE id = 'em1'")
                .fetch_one(pool)
                .await
                .unwrap();
        assert_eq!(em_human, alice_id);
        let em_tag: String =
            sqlx::query_scalar("SELECT target_id FROM entity_mentions WHERE id = 'em2'")
                .fetch_one(pool)
                .await
                .unwrap();
        assert_eq!(em_tag, "uuid-alice"); // target_type guard: untouched

        // speaker_hints_json rewritten, unknown shapes preserved, invalid
        // JSON left verbatim.
        let hints: String =
            sqlx::query_scalar("SELECT speaker_hints_json FROM transcripts WHERE id = 't1'")
                .fetch_one(pool)
                .await
                .unwrap();
        assert!(hints.contains(&alice_id));
        assert!(hints.contains("\"unknown\":7"));
        let bad: String =
            sqlx::query_scalar("SELECT speaker_hints_json FROM transcripts WHERE id = 't2'")
                .fetch_one(pool)
                .await
                .unwrap();
        assert_eq!(bad, "not json at all");

        // Participants: human-backed and email-backed rows recomputed, the
        // display-name-only row keeps its UUID.
        let p1: Option<String> = sqlx::query_scalar(
            "SELECT id FROM session_participants WHERE session_id = 'session-1'",
        )
        .fetch_optional(pool)
        .await
        .unwrap();
        assert_eq!(p1, Some(p1_id.clone()));
        let p1_human: String =
            sqlx::query_scalar("SELECT human_id FROM session_participants WHERE id = ?")
                .bind(&p1_id)
                .fetch_one(pool)
                .await
                .unwrap();
        assert_eq!(p1_human, alice_id);
        let p2: Option<String> = sqlx::query_scalar(
            "SELECT id FROM session_participants WHERE session_id = 'session-2' AND email = 'bob@example.com'",
        )
        .fetch_optional(pool)
        .await
        .unwrap();
        assert_eq!(p2, Some(p2_id));
        let p3: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM session_participants WHERE id = 'uuid-p3'")
                .fetch_one(pool)
                .await
                .unwrap();
        assert_eq!(p3, 1);

        // Orgs: duplicate names collapsed, survivor renamed,
        // humans.organization_id redirected to the survivor.
        let orgs: Vec<(String, Option<String>)> =
            sqlx::query_as("SELECT id, deleted_at FROM organizations ORDER BY id")
                .fetch_all(pool)
                .await
                .unwrap();
        assert!(
            orgs.iter()
                .any(|(id, deleted)| id == &acme_id && deleted.is_none())
        );
        assert!(
            orgs.iter()
                .any(|(id, deleted)| id == "uuid-acme-2" && deleted.is_some())
        );
        let human_org: String =
            sqlx::query_scalar("SELECT organization_id FROM humans WHERE id = ?")
                .bind(&alice_id)
                .fetch_one(pool)
                .await
                .unwrap();
        assert_eq!(human_org, acme_id);

        // Marker set; a second run is a no-op.
        let marker: Option<String> =
            sqlx::query_scalar("SELECT value_json FROM app_settings WHERE id = ?")
                .bind(ID_SCHEME_MARKER)
                .fetch_optional(pool)
                .await
                .unwrap();
        assert_eq!(marker, Some("true".to_string()));

        backfill_deterministic_ids(pool).await.unwrap();

        let unchanged: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM session_participants WHERE id = 'uuid-p3'")
                .fetch_one(pool)
                .await
                .unwrap();
        assert_eq!(unchanged, 1);
        let alice_still: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM humans WHERE id = ?")
            .bind(&alice_id)
            .fetch_one(pool)
            .await
            .unwrap();
        assert_eq!(alice_still, 1);
    }
}

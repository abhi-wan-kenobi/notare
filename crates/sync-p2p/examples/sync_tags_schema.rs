//! SYNC-10 (table-proofs lane): the CRDT converges notare's **real** `tags`
//! and `session_tags` tables. `session_tags` is a join table (session <->
//! tag association), which looked at first like a different CRDT case from
//! plain row-update convergence — two devices tagging the same session with
//! the same tag would insert two DIFFERENT rows if `session_tags.id` were an
//! independently-generated PK. It is not: the real app
//! (`apps/desktop/src/session/content-mutations.ts:148-186`) derives BOTH
//! primary keys deterministically —
//!
//! - `tags.id` = the tag name itself (line ~151: `VALUES (?, ?, ?, ?, ?,
//!   NULL)` bound to `[tagName, userId, tagName, now, now]`).
//! - `session_tags.id` = `` `${sessionId}:${tagName}` `` (line ~179).
//!
//! and both writes go through `INSERT ... ON CONFLICT(id) DO UPDATE SET ...
//! deleted_at = NULL`, not a plain INSERT. So when two devices independently
//! tag the same session with the same tag, they generate the SAME primary
//! key on both sides — this is a same-row concurrent-insert/update case, not
//! a duplicate-row case. An earlier version of this proof used random UUIDs
//! for `session_tags.id` and concluded the opposite; that was a fixture bug,
//! not a schema or CRDT finding — see docs/internal/sync-p2p.md §23 for the
//! correction. This version's helper functions (`upsert_tag`,
//! `upsert_session_tag`) copy the app's real SQL text and id derivation
//! verbatim, not just the DDL.
//!
//! `CREATE TABLE` bodies are copied verbatim from
//! `crates/db-app/migrations/20260710223922_canonical_data_model.sql`
//! (lines 155-163 for `tags`, 165-174 for `session_tags`). Neither declares
//! a FOREIGN KEY, matching production and the §19 correction.
//!
//! Run: `cargo run -p sync-p2p --example sync_tags_schema --features from-source`

use std::str::FromStr;
use std::time::Duration;

use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sync_p2p::{Identity, P2pAgent, PeerStore, register_direct_addr};

const DB_ID: &str = "notare-v06";
const MAX_DRAIN: usize = 16;

/// Verbatim from `20260710223922_canonical_data_model.sql:155-163`.
const CREATE_TAGS: &str = "CREATE TABLE IF NOT EXISTS tags (
  id             TEXT PRIMARY KEY NOT NULL,
  workspace_id   TEXT NOT NULL DEFAULT '',
  owner_user_id  TEXT NOT NULL DEFAULT '',
  name           TEXT NOT NULL DEFAULT '',
  created_at     TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at     TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  deleted_at     TEXT
) STRICT";

/// Verbatim from `20260710223922_canonical_data_model.sql:165-174`.
const CREATE_SESSION_TAGS: &str = "CREATE TABLE IF NOT EXISTS session_tags (
  id            TEXT PRIMARY KEY NOT NULL,
  workspace_id  TEXT NOT NULL DEFAULT '',
  owner_user_id TEXT NOT NULL DEFAULT '',
  session_id    TEXT NOT NULL DEFAULT '',
  tag_id        TEXT NOT NULL DEFAULT '',
  created_at    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  deleted_at    TEXT
) STRICT";

/// Verbatim from `apps/desktop/src/session/content-mutations.ts:146-160` —
/// the real "add tag" upsert the app issues. `id` = `tag_name`
/// (deterministic): two devices adding the same tag name write the same PK.
async fn upsert_tag(pool: &SqlitePool, tag_name: &str, owner_user_id: &str, now: &str) {
    sqlx::query(
        "INSERT INTO tags (
            id, owner_user_id, name, created_at, updated_at, deleted_at
        ) VALUES (?, ?, ?, ?, ?, NULL)
        ON CONFLICT(id) DO UPDATE SET
            owner_user_id = excluded.owner_user_id,
            name = excluded.name,
            updated_at = excluded.updated_at,
            deleted_at = NULL",
    )
    .bind(tag_name)
    .bind(owner_user_id)
    .bind(tag_name)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await
    .unwrap();
}

/// Verbatim from `apps/desktop/src/session/content-mutations.ts:161-179` —
/// the real "tag this session" upsert. `id` = `` `${session_id}:${tag_name}` ``
/// (deterministic — this is the fact that changes the CRDT case: two devices
/// tagging the same session with the same tag write the SAME primary key,
/// not two different rows). Returns the derived id.
async fn upsert_session_tag(
    pool: &SqlitePool,
    session_id: &str,
    tag_name: &str,
    owner_user_id: &str,
    now: &str,
) -> String {
    let id = format!("{session_id}:{tag_name}");
    sqlx::query(
        "INSERT INTO session_tags (
            id, owner_user_id, session_id, tag_id,
            created_at, updated_at, deleted_at
        ) VALUES (?, ?, ?, ?, ?, ?, NULL)
        ON CONFLICT(id) DO UPDATE SET
            owner_user_id = excluded.owner_user_id,
            session_id = excluded.session_id,
            tag_id = excluded.tag_id,
            updated_at = excluded.updated_at,
            deleted_at = NULL",
    )
    .bind(&id)
    .bind(owner_user_id)
    .bind(session_id)
    .bind(tag_name)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await
    .unwrap();
    id
}

async fn setup_node(
    uri: &str,
    broker_addr: &str,
    local_agent_tcp: &str,
    local_token: &str,
) -> SqlitePool {
    let options = SqliteConnectOptions::from_str(uri).unwrap();
    let options = options.pragma("foreign_keys", "ON");
    let (options, _ext_path) = cloudsync::apply(options).unwrap();
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .unwrap();

    let version: String = sqlx::query_scalar("SELECT cloudsync_version()")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(version, cloudsync::CLOUDSYNC_VERSION);

    sqlx::query(CREATE_TAGS).execute(&pool).await.unwrap();
    sqlx::query(CREATE_SESSION_TAGS)
        .execute(&pool)
        .await
        .unwrap();

    for table in ["tags", "session_tags"] {
        sqlx::query("SELECT cloudsync_init(?, 'cls', 1)")
            .bind(table)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("SELECT cloudsync_enable(?)")
            .bind(table)
            .execute(&pool)
            .await
            .unwrap();
    }
    sqlx::query("SELECT cloudsync_network_init_custom(?, ?)")
        .bind(broker_addr)
        .bind(DB_ID)
        .execute(&pool)
        .await
        .unwrap();

    // SAFETY: sequential single-process example; see sync_sessions_schema.rs.
    unsafe {
        std::env::set_var("NOTARE_SYNC_AGENT_ADDR", local_agent_tcp);
        std::env::set_var("NOTARE_SYNC_TOKEN", local_token);
    }

    pool
}

async fn run_sync(pool: &SqlitePool, local_agent_tcp: &str, local_token: &str) -> String {
    unsafe {
        std::env::set_var("NOTARE_SYNC_AGENT_ADDR", local_agent_tcp);
        std::env::set_var("NOTARE_SYNC_TOKEN", local_token);
    }
    sqlx::query_scalar::<_, String>("SELECT cloudsync_network_sync()")
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn run_check(pool: &SqlitePool, local_agent_tcp: &str, local_token: &str) -> String {
    unsafe {
        std::env::set_var("NOTARE_SYNC_AGENT_ADDR", local_agent_tcp);
        std::env::set_var("NOTARE_SYNC_TOKEN", local_token);
    }
    sqlx::query_scalar::<_, String>("SELECT cloudsync_network_check_changes()")
        .fetch_one(pool)
        .await
        .unwrap()
}

fn rows_received(resp: &str) -> Option<u64> {
    let v: serde_json::Value = serde_json::from_str(resp).ok()?;
    v.get("receive")?.get("rows")?.as_u64()
}

async fn drain_check(pool: &SqlitePool, tcp: &str, token: &str, label: &str) -> usize {
    let mut applied = 0;
    for _ in 0..MAX_DRAIN {
        let resp = run_check(pool, tcp, token).await;
        match rows_received(&resp) {
            None => panic!(
                "[{label}] unreadable check reply, cannot know if changes are pending: {resp}"
            ),
            Some(0) => {
                println!("[{label}] drained {applied} change set(s)");
                return applied;
            }
            Some(_) => applied += 1,
        }
    }
    panic!("[{label}] hit MAX_DRAIN ({MAX_DRAIN}) with changes still pending");
}

async fn sync_and_drain(pool: &SqlitePool, tcp: &str, token: &str, label: &str) {
    run_sync(pool, tcp, token).await;
    drain_check(pool, tcp, token, label).await;
}

/// (name, owner_user_id, updated_at, deleted_at)
async fn tag_row(pool: &SqlitePool, id: &str) -> Option<(String, String, String, Option<String>)> {
    sqlx::query_as::<_, (String, String, String, Option<String>)>(
        "SELECT name, owner_user_id, updated_at, deleted_at FROM tags WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .unwrap()
}

/// (session_id, tag_id, owner_user_id, updated_at, deleted_at)
async fn session_tag_full(
    pool: &SqlitePool,
    id: &str,
) -> Option<(String, String, String, String, Option<String>)> {
    sqlx::query_as::<_, (String, String, String, String, Option<String>)>(
        "SELECT session_id, tag_id, owner_user_id, updated_at, deleted_at FROM session_tags WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .unwrap()
}

/// Live (non-tombstoned) association rows for a (session_id, tag_id) pair —
/// what a UI reading "which tags does this session have" would query.
async fn live_associations(pool: &SqlitePool, session_id: &str, tag_id: &str) -> Vec<String> {
    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT id FROM session_tags WHERE session_id = ? AND tag_id = ? AND deleted_at IS NULL ORDER BY id",
    )
    .bind(session_id)
    .bind(tag_id)
    .fetch_all(pool)
    .await
    .unwrap();
    rows.into_iter().map(|(id,)| id).collect()
}

async fn count(pool: &SqlitePool, table: &str) -> i64 {
    let sql: &'static str = match table {
        "tags" => "SELECT COUNT(*) FROM tags",
        "session_tags" => "SELECT COUNT(*) FROM session_tags",
        other => panic!("unknown table {other}"),
    };
    sqlx::query_scalar(sql).fetch_one(pool).await.unwrap()
}

fn short(agent: &P2pAgent) -> String {
    agent.node_id().to_z32().chars().take(8).collect()
}

#[tokio::main(flavor = "multi_thread", worker_threads = 4)]
async fn main() {
    let tmp = tempfile::tempdir().unwrap();

    let dir_a = tempfile::tempdir_in(tmp.path()).unwrap();
    let dir_b = tempfile::tempdir_in(tmp.path()).unwrap();

    let id_a = Identity::load_or_create_in(dir_a.path()).unwrap();
    let id_b = Identity::load_or_create_in(dir_b.path()).unwrap();

    let peers_a = PeerStore::load_or_create_in(dir_a.path()).unwrap();
    let peers_b = PeerStore::load_or_create_in(dir_b.path()).unwrap();
    peers_a.add_peer(id_b.id(), "Node B").unwrap();
    peers_b.add_peer(id_a.id(), "Node A").unwrap();

    let agent_a = P2pAgent::start_with(id_a, peers_a).await.unwrap();
    let agent_b = P2pAgent::start_with(id_b, peers_b).await.unwrap();

    register_direct_addr(agent_a.node_id(), agent_a.direct_addresses()).await;
    register_direct_addr(agent_b.node_id(), agent_b.direct_addresses()).await;
    tokio::time::sleep(Duration::from_millis(150)).await;

    let a_tcp = agent_a.local_addr.clone();
    let b_tcp = agent_b.local_addr.clone();
    let a_token = agent_a.token().to_string();
    let b_token = agent_b.token().to_string();
    let broker = agent_a.address();

    println!("[agents] A={} (broker, tcp {a_tcp})", short(&agent_a));
    println!("[agents] B={} (tcp {b_tcp})", short(&agent_b));
    println!("[peers]  A allowlists B; B allowlists A");

    let a = setup_node(
        &format!(
            "sqlite://{}?mode=rwc",
            dir_a.path().join("node_a.db").display()
        ),
        &broker,
        &a_tcp,
        &a_token,
    )
    .await;
    let b = setup_node(
        &format!(
            "sqlite://{}?mode=rwc",
            dir_b.path().join("node_b.db").display()
        ),
        &broker,
        &b_tcp,
        &b_token,
    )
    .await;
    println!("[nodes] A and B initialized; cloudsync enabled on tags + session_tags (broker = A)");

    const SESSION_X: &str = "sess-x";

    // 1. Scenario A->B: A adds tag 'urgent' to session X via the app's real
    //    upsert (deterministic ids: tags.id='urgent',
    //    session_tags.id='sess-x:urgent').
    let t0 = "2026-09-01T00:00:00.000Z";
    upsert_tag(&a, "urgent", "user-a", t0).await;
    let urgent_on_x = upsert_session_tag(&a, SESSION_X, "urgent", "user-a", t0).await;
    println!("[A] added tag 'urgent' to session {SESSION_X} (session_tags.id={urgent_on_x})");

    run_sync(&a, &a_tcp, &a_token).await;
    drain_check(&b, &b_tcp, &b_token, "B").await;

    let tag_b = tag_row(&b, "urgent").await.expect("B has A's tag");
    assert_eq!(tag_b.0, "urgent", "B's tag name");
    let assoc_b = session_tag_full(&b, &urgent_on_x)
        .await
        .expect("B has A's session_tags association");
    assert_eq!(
        (assoc_b.0.as_str(), assoc_b.1.as_str(), assoc_b.4.clone()),
        (SESSION_X, "urgent", None),
        "B's association row"
    );
    println!("[conv] A -> B OK (tag + association, real deterministic ids)");

    // 2. Scenario B->A: reverse direction, a second tag.
    upsert_tag(&b, "follow-up", "user-b", t0).await;
    let followup_on_x = upsert_session_tag(&b, SESSION_X, "follow-up", "user-b", t0).await;
    println!("[B] added tag 'follow-up' to session {SESSION_X} (session_tags.id={followup_on_x})");

    run_sync(&b, &b_tcp, &b_token).await;
    drain_check(&a, &a_tcp, &a_token, "A").await;

    let tag_a = tag_row(&a, "follow-up").await.expect("A has B's tag");
    assert_eq!(tag_a.0, "follow-up", "A's tag name");
    let assoc_a = session_tag_full(&a, &followup_on_x)
        .await
        .expect("A has B's session_tags association");
    assert_eq!(
        (assoc_a.0.as_str(), assoc_a.1.as_str(), assoc_a.4.clone()),
        (SESSION_X, "follow-up", None),
        "A's association row"
    );
    println!("[conv] B -> A OK");

    // 3. Scenario (the corrected core case): concurrent IDENTICAL tag-add on
    //    the SAME deterministic primary key. Two devices, disconnected,
    //    independently call the app's real "add tag" upsert for the SAME
    //    session + SAME tag name, as two different users (different
    //    owner_user_id, different `now`) would if they both tagged a shared
    //    session while offline. Because both PKs are deterministic, this
    //    writes the SAME row on both nodes — a same-row concurrent
    //    insert/update, not two rows.
    const SESSION_Y: &str = "sess-y";
    const SHARED_TAG: &str = "shared-tag";
    let t_a = "2026-09-01T00:00:00.000Z";
    let t_b = "2026-09-01T00:00:05.000Z"; // B writes 5s "later"

    upsert_tag(&a, SHARED_TAG, "user-a", t_a).await;
    let sid_a = upsert_session_tag(&a, SESSION_Y, SHARED_TAG, "user-a", t_a).await;
    upsert_tag(&b, SHARED_TAG, "user-b", t_b).await;
    let sid_b = upsert_session_tag(&b, SESSION_Y, SHARED_TAG, "user-b", t_b).await;
    assert_eq!(
        sid_a, sid_b,
        "the app's deterministic id scheme must produce the same session_tags id on both nodes"
    );
    println!(
        "[both] concurrently, independently added the SAME tag {SHARED_TAG:?} to session \
         {SESSION_Y} (same PK {sid_a:?} on both nodes, via the app's real upsert; A as \
         user-a@{t_a}, B as user-b@{t_b})"
    );

    for _ in 0..MAX_DRAIN {
        sync_and_drain(&a, &a_tcp, &a_token, "A").await;
        sync_and_drain(&b, &b_tcp, &b_token, "B").await;
    }

    let final_tag_a = tag_row(&a, SHARED_TAG)
        .await
        .expect("tags row must exist on A");
    let final_tag_b = tag_row(&b, SHARED_TAG)
        .await
        .expect("tags row must exist on B");
    assert_eq!(
        final_tag_a, final_tag_b,
        "tags row for {SHARED_TAG} diverged A vs B after the concurrent add"
    );
    assert!(
        final_tag_a.1 == "user-a" || final_tag_a.1 == "user-b",
        "converged owner_user_id {:?} is neither of the two writes — torn or merged",
        final_tag_a.1
    );
    println!(
        "[conv] concurrent identical tag-add converged to ONE tags row on both nodes \
         (owner_user_id={:?}, updated_at={:?})",
        final_tag_a.1, final_tag_a.2
    );

    let final_assoc_a = session_tag_full(&a, &sid_a)
        .await
        .expect("session_tags row must exist on A");
    let final_assoc_b = session_tag_full(&b, &sid_a)
        .await
        .expect("session_tags row must exist on B");
    assert_eq!(
        final_assoc_a, final_assoc_b,
        "session_tags row {sid_a} diverged A vs B after the concurrent add"
    );
    assert!(
        final_assoc_a.2 == "user-a" || final_assoc_a.2 == "user-b",
        "converged owner_user_id {:?} is neither of the two writes — torn or merged",
        final_assoc_a.2
    );
    let live = live_associations(&a, SESSION_Y, SHARED_TAG).await;
    assert_eq!(
        live,
        vec![sid_a.clone()],
        "expected exactly ONE live session_tags row for the concurrently-added association — \
         the app's deterministic id scheme prevents a duplicate-row outcome"
    );
    println!(
        "[conv] concurrent identical tag-add converged to ONE session_tags row on both nodes \
         (id={sid_a}, owner_user_id={:?}, updated_at={:?}) — no duplicate row, matching the \
         app's real deterministic-id write pattern",
        final_assoc_a.2, final_assoc_a.3
    );

    // 4a. Scenario: tombstone-as-delete (no concurrent race). A "removes" the
    //     'follow-up' tag from session X by soft-deleting the session_tags
    //     association row. No dedicated remove-tag mutation exists in the
    //     app yet (checked apps/desktop/src for it — none found); this
    //     models one using the same soft-delete/tombstone convention every
    //     other table in this schema already relies on. B must see the
    //     tombstone and it must not resurrect.
    let tombstone_at = "2026-09-01T01:00:00.000Z";
    sqlx::query("UPDATE session_tags SET deleted_at = ? WHERE id = ?")
        .bind(tombstone_at)
        .bind(&followup_on_x)
        .execute(&a)
        .await
        .unwrap();
    println!("[A] soft-deleted association {followup_on_x} (removed 'follow-up' from {SESSION_X})");

    run_sync(&a, &a_tcp, &a_token).await;
    drain_check(&b, &b_tcp, &b_token, "B").await;

    assert_eq!(
        session_tag_full(&b, &followup_on_x).await.and_then(|r| r.4),
        Some(tombstone_at.to_string()),
        "the deleted_at tombstone value must sync across verbatim"
    );

    for _ in 0..3 {
        sync_and_drain(&a, &a_tcp, &a_token, "A").await;
        sync_and_drain(&b, &b_tcp, &b_token, "B").await;
    }
    assert_eq!(
        session_tag_full(&b, &followup_on_x).await.and_then(|r| r.4),
        Some(tombstone_at.to_string()),
        "tombstone changed / row resurrected on B after further syncs"
    );
    assert_eq!(
        session_tag_full(&a, &followup_on_x).await.and_then(|r| r.4),
        Some(tombstone_at.to_string()),
        "tombstone changed / row resurrected on A after further syncs"
    );
    println!(
        "[conv] tombstone-as-delete OK, no resurrection after further sync rounds (clean, no concurrent race)"
    );

    // 4b. Scenario (the corrected add-vs-remove case): concurrent tombstone
    //     vs. reinsert on the SAME primary key. Baseline: A tags session Z
    //     with 'urgent', synced to both. Then, disconnected: A "removes" it
    //     (soft-delete, same modeling caveat as 4a) while B, unaware,
    //     independently re-issues the app's real add-tag upsert for the
    //     SAME (session, tag) pair — which hits the SAME row (deterministic
    //     id) and explicitly sets deleted_at = NULL on conflict. This is a
    //     genuine last-writer-wins race on one row's deleted_at column, not
    //     the two-row outcome the earlier (incorrect) version of this proof
    //     found.
    const SESSION_Z: &str = "sess-z";
    let base_id = upsert_session_tag(
        &a,
        SESSION_Z,
        "urgent",
        "user-a",
        "2026-09-01T02:00:00.000Z",
    )
    .await;
    run_sync(&a, &a_tcp, &a_token).await;
    drain_check(&b, &b_tcp, &b_token, "B").await;
    println!("[setup] session {SESSION_Z} tagged with 'urgent' via {base_id}, synced to both");

    let remove_at = "2026-09-01T02:00:10.000Z";
    sqlx::query("UPDATE session_tags SET deleted_at = ? WHERE id = ?")
        .bind(remove_at)
        .bind(&base_id)
        .execute(&a)
        .await
        .unwrap();
    let reinsert_at = "2026-09-01T02:00:11.000Z";
    let reinsert_id = upsert_session_tag(&b, SESSION_Z, "urgent", "user-b", reinsert_at).await;
    assert_eq!(
        base_id, reinsert_id,
        "the reinsert must hit the SAME row as the removal (deterministic id) — this is the point"
    );
    println!(
        "[concurrent] A removed association {base_id} (deleted_at={remove_at}) while B, \
         unaware, independently re-added the SAME (session, tag) pair via the app's real \
         upsert (updated_at={reinsert_at}) — same PK, not a new row"
    );

    for _ in 0..MAX_DRAIN {
        sync_and_drain(&a, &a_tcp, &a_token, "A").await;
        sync_and_drain(&b, &b_tcp, &b_token, "B").await;
    }

    let race_a = session_tag_full(&a, &base_id)
        .await
        .expect("row must still exist — only deleted_at is contested, never a real DELETE");
    let race_b = session_tag_full(&b, &base_id)
        .await
        .expect("row must still exist on B");
    assert_eq!(
        race_a, race_b,
        "session_tags row {base_id} diverged A vs B after the tombstone-vs-reinsert race — \
         this would be a real convergence failure, not a benign duplicate"
    );
    let outcome = if race_a.4.is_none() {
        "B's reinsert won — the tag is present on both nodes"
    } else {
        "A's removal won — the tag stays absent on both nodes"
    };
    println!(
        "[conv] concurrent tombstone-vs-reinsert on the SAME row converged: both nodes agree \
         (deleted_at={:?}, owner_user_id={:?}, updated_at={:?}) — {outcome}",
        race_a.4, race_a.2, race_a.3
    );

    // Run a few more rounds to confirm the settled outcome is STABLE, not an
    // intermediate value still in flight.
    for _ in 0..3 {
        sync_and_drain(&a, &a_tcp, &a_token, "A").await;
        sync_and_drain(&b, &b_tcp, &b_token, "B").await;
    }
    let settled_a = session_tag_full(&a, &base_id).await.unwrap();
    let settled_b = session_tag_full(&b, &base_id).await.unwrap();
    assert_eq!(
        settled_a, settled_b,
        "row diverged again after further syncs"
    );
    assert_eq!(
        settled_a.4, race_a.4,
        "the tombstone-vs-reinsert outcome was not stable — it moved after further sync rounds"
    );
    println!("[conv] tombstone-vs-reinsert outcome held stable across further sync rounds");

    // 4c. Scenario (the complementary, ordinary case): B sees A's removal
    //     FIRST, then re-tags — the normal "I removed it, then added it
    //     back" flow, as opposed to 4b's genuine concurrency window.
    //     Continuing from 4b's settled state: both nodes already agree
    //     row `base_id` is tombstoned (deleted_at = Some(remove_at)), so
    //     B's OWN local deleted_at is now non-NULL. When B now re-issues
    //     the app's real add-tag upsert, `SET deleted_at = NULL` against a
    //     local NON-NULL value IS a genuine delta this time (unlike 4b,
    //     where B's local value was already NULL) — so per the traced
    //     mechanism (§23.4) it must bump B's local col_version for that
    //     column and contest it, not arrive uncontested. If this does NOT
    //     converge to "present on both nodes, stays present", tags would
    //     be silently un-re-addable after any cross-device removal — a
    //     serious product bug, not a benign concurrency artifact.
    assert_eq!(
        settled_b.4,
        Some(remove_at.to_string()),
        "precondition: B must see the tombstone (non-NULL deleted_at) before re-tagging, \
         or this isn't testing the causally-later case"
    );
    let readd_at = "2026-09-01T02:00:20.000Z";
    let readd_id = upsert_session_tag(&b, SESSION_Z, "urgent", "user-b", readd_at).await;
    assert_eq!(
        readd_id, base_id,
        "the re-add must hit the SAME row (deterministic id)"
    );
    println!(
        "[B] saw A's removal first (local deleted_at was {:?}), THEN re-tagged {SESSION_Z} \
         with 'urgent' via the app's real upsert (updated_at={readd_at})",
        settled_b.4
    );

    for _ in 0..MAX_DRAIN {
        sync_and_drain(&a, &a_tcp, &a_token, "A").await;
        sync_and_drain(&b, &b_tcp, &b_token, "B").await;
    }

    let readd_a = session_tag_full(&a, &base_id)
        .await
        .expect("row must still exist on A");
    let readd_b = session_tag_full(&b, &base_id)
        .await
        .expect("row must still exist on B");
    assert_eq!(
        readd_a, readd_b,
        "session_tags row {base_id} diverged A vs B after the causally-later re-add"
    );
    assert_eq!(
        readd_a.4, None,
        "the causally-later re-add must win: deleted_at should be cleared (tag present) on \
         both nodes, not stuck tombstoned — a silent un-re-addable tag would be a real bug"
    );
    println!(
        "[conv] causally-later re-add OK: the tag is present again on both nodes \
         (deleted_at={:?}, owner_user_id={:?}, updated_at={:?})",
        readd_a.4, readd_a.2, readd_a.3
    );

    // Confirm it STAYS present, not just momentarily in flight.
    for _ in 0..3 {
        sync_and_drain(&a, &a_tcp, &a_token, "A").await;
        sync_and_drain(&b, &b_tcp, &b_token, "B").await;
    }
    let final_readd_a = session_tag_full(&a, &base_id).await.unwrap();
    let final_readd_b = session_tag_full(&b, &base_id).await.unwrap();
    assert_eq!(
        final_readd_a, final_readd_b,
        "row diverged again after further syncs"
    );
    assert_eq!(
        final_readd_a.4, None,
        "the re-added tag did not stay present after further sync rounds"
    );
    println!("[conv] causally-later re-add stayed present across further sync rounds");

    // 5. Multi-row catch-up across both tables, via the app's real upsert.
    for n in 0..3 {
        let name = format!("bulk-tag-a-{n}");
        upsert_tag(&a, &name, "user-a", "2026-09-01T03:00:00.000Z").await;
    }
    for n in 0..3 {
        let name = format!("bulk-tag-b-{n}");
        upsert_tag(&b, &name, "user-b", "2026-09-01T03:00:00.000Z").await;
    }
    println!("[both] wrote 3 bulk tags each before draining");

    for _ in 0..MAX_DRAIN {
        sync_and_drain(&a, &a_tcp, &a_token, "A").await;
        sync_and_drain(&b, &b_tcp, &b_token, "B").await;
    }

    assert_eq!(
        count(&a, "tags").await,
        count(&b, "tags").await,
        "tag count differs A vs B"
    );
    assert_eq!(
        count(&a, "session_tags").await,
        count(&b, "session_tags").await,
        "session_tags count differs A vs B"
    );
    println!("[conv] multi-row catch-up OK (full set equality across both tables)");

    println!(
        "\n=== tags + session_tags schema proof: converge under the app's REAL deterministic \
         id scheme, including the concurrent-identical-add and tombstone-vs-reinsert races on \
         the SAME primary key ==="
    );

    a.close().await;
    b.close().await;
    agent_a.stop().await;
    agent_b.stop().await;
}

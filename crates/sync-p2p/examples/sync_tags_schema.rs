//! SYNC-10 (table-proofs lane): the CRDT converges notare's **real** `tags`
//! and `session_tags` tables. `session_tags` is a join table (session <->
//! tag association), which is a genuinely different CRDT case from plain
//! row-update convergence: the "same edit" a user makes on two devices is
//! not an UPDATE to a shared row but an INSERT of a *new* row referencing
//! the same (session_id, tag_id) pair, because `session_tags.id` is its own
//! independently-generated TEXT PK, not a composite key of
//! `(session_id, tag_id)`. There is no UNIQUE constraint on that pair in the
//! migration either (verified: no `UNIQUE` on `session_tags` in
//! `20260710223922_canonical_data_model.sql`). This example both proves
//! CRDT-level convergence (no torn/lost writes, no resurrection) AND
//! documents the resulting product-level finding: concurrent identical
//! "add this tag" actions on two offline devices converge to **two** rows
//! for the same association, not one.
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

/// Verbatim from `20260710223922_canonical_data_model.sql:165-174`. No
/// UNIQUE constraint on (session_id, tag_id) — the same shape production has.
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

async fn tag_name(pool: &SqlitePool, id: &str) -> Option<String> {
    sqlx::query_scalar("SELECT name FROM tags WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await
        .unwrap()
}

async fn tag_deleted_at(pool: &SqlitePool, id: &str) -> Option<Option<String>> {
    sqlx::query_scalar("SELECT deleted_at FROM tags WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await
        .unwrap()
}

async fn session_tag_row(pool: &SqlitePool, id: &str) -> Option<(String, String, Option<String>)> {
    sqlx::query_as::<_, (String, String, Option<String>)>(
        "SELECT session_id, tag_id, deleted_at FROM session_tags WHERE id = ?",
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

    // 1. Scenario A->B: A creates a tag and associates it with a session.
    const TAG1: &str = "11111111-1111-1111-1111-111111111111";
    const ST1: &str = "22222222-2222-2222-2222-222222222222";
    const SESSION_X: &str = "sess-x";
    sqlx::query("INSERT INTO tags (id, name) VALUES (?, 'urgent')")
        .bind(TAG1)
        .execute(&a)
        .await
        .unwrap();
    sqlx::query("INSERT INTO session_tags (id, session_id, tag_id) VALUES (?, ?, ?)")
        .bind(ST1)
        .bind(SESSION_X)
        .bind(TAG1)
        .execute(&a)
        .await
        .unwrap();
    println!("[A] created tag 'urgent' and tagged session {SESSION_X}");

    run_sync(&a, &a_tcp, &a_token).await;
    drain_check(&b, &b_tcp, &b_token, "B").await;

    assert_eq!(
        tag_name(&b, TAG1).await,
        Some("urgent".into()),
        "B has A's tag"
    );
    assert_eq!(
        session_tag_row(&b, ST1).await,
        Some((SESSION_X.into(), TAG1.into(), None)),
        "B has A's session_tags association"
    );
    println!("[conv] A -> B OK (tag + association)");

    // 2. Scenario B->A: reverse direction, a second tag.
    const TAG2: &str = "33333333-3333-3333-3333-333333333333";
    const ST2: &str = "44444444-4444-4444-4444-444444444444";
    sqlx::query("INSERT INTO tags (id, name) VALUES (?, 'follow-up')")
        .bind(TAG2)
        .execute(&b)
        .await
        .unwrap();
    sqlx::query("INSERT INTO session_tags (id, session_id, tag_id) VALUES (?, ?, ?)")
        .bind(ST2)
        .bind(SESSION_X)
        .bind(TAG2)
        .execute(&b)
        .await
        .unwrap();
    println!("[B] created tag 'follow-up' and tagged session {SESSION_X}");

    run_sync(&b, &b_tcp, &b_token).await;
    drain_check(&a, &a_tcp, &a_token, "A").await;

    assert_eq!(
        tag_name(&a, TAG2).await,
        Some("follow-up".into()),
        "A has B's tag"
    );
    assert_eq!(
        session_tag_row(&a, ST2).await,
        Some((SESSION_X.into(), TAG2.into(), None)),
        "A has B's session_tags association"
    );
    println!("[conv] B -> A OK");

    // 3. Scenario: disconnected concurrent UPDATE of the same tag's `name`
    //    converges conflict-free.
    sqlx::query("UPDATE tags SET name = 'urgent-renamed-by-A' WHERE id = ?")
        .bind(TAG1)
        .execute(&a)
        .await
        .unwrap();
    sqlx::query("UPDATE tags SET name = 'urgent-renamed-by-B' WHERE id = ?")
        .bind(TAG1)
        .execute(&b)
        .await
        .unwrap();
    println!("[both] renamed tag {TAG1} concurrently while disconnected");

    for _ in 0..MAX_DRAIN {
        sync_and_drain(&a, &a_tcp, &a_token, "A").await;
        sync_and_drain(&b, &b_tcp, &b_token, "B").await;
        let (na, nb) = (tag_name(&a, TAG1).await, tag_name(&b, TAG1).await);
        if na == nb {
            sync_and_drain(&a, &a_tcp, &a_token, "A").await;
            sync_and_drain(&b, &b_tcp, &b_token, "B").await;
            let (fa, fb) = (tag_name(&a, TAG1).await, tag_name(&b, TAG1).await);
            assert_eq!(fa, fb, "tag name diverged A vs B after a settling round");
            assert_eq!(
                fa, na,
                "tag name was not stable — agreed on {na:?} then moved to {fa:?}"
            );
            let settled = fa.expect("row vanished during settle");
            assert!(
                settled == "urgent-renamed-by-A" || settled == "urgent-renamed-by-B",
                "converged value {settled:?} is neither of the two writes — torn or merged"
            );
            println!(
                "[conv] concurrent tag rename converged and held (name = {settled:?} on both)"
            );
            break;
        }
    }

    // 4. Scenario: tombstone-as-delete. A soft-deletes tag TAG2 (deleted_at)
    //    and hard-deletes the association row ST2 that referenced it; B must
    //    see the tombstone and the row must not resurrect.
    sqlx::query("UPDATE tags SET deleted_at = '2026-09-01T00:00:00Z' WHERE id = ?")
        .bind(TAG2)
        .execute(&a)
        .await
        .unwrap();
    sqlx::query("DELETE FROM session_tags WHERE id = ?")
        .bind(ST2)
        .execute(&a)
        .await
        .unwrap();
    println!("[A] soft-deleted tag {TAG2} and hard-deleted association {ST2}");

    run_sync(&a, &a_tcp, &a_token).await;
    drain_check(&b, &b_tcp, &b_token, "B").await;

    assert_eq!(
        tag_deleted_at(&b, TAG2).await,
        Some(Some("2026-09-01T00:00:00Z".into())),
        "the deleted_at tombstone value must sync across verbatim"
    );
    assert!(
        session_tag_row(&b, ST2).await.is_none(),
        "the deleted association must not resurrect on B"
    );

    for _ in 0..3 {
        sync_and_drain(&a, &a_tcp, &a_token, "A").await;
        sync_and_drain(&b, &b_tcp, &b_token, "B").await;
    }
    assert!(
        session_tag_row(&b, ST2).await.is_none(),
        "deleted association resurrected on B after further syncs"
    );
    assert!(
        session_tag_row(&a, ST2).await.is_none(),
        "deleted association resurrected on A after further syncs"
    );
    println!("[conv] tombstone-as-delete OK, no resurrection after further sync rounds");

    // 5a. Scenario: concurrent IDENTICAL add — both devices, offline,
    //     independently associate session Y with the SAME tag (TAG1). This
    //     is the join-table-specific case: since session_tags.id is its own
    //     PK (not a composite key of session_id+tag_id, and there is no
    //     UNIQUE constraint on that pair — verified against the migration),
    //     the two inserts are two DIFFERENT rows, not a conflict on one row.
    const SESSION_Y: &str = "sess-y";
    const ST3_A: &str = "55555555-5555-5555-5555-555555555555";
    const ST3_B: &str = "66666666-6666-6666-6666-666666666666";
    sqlx::query("INSERT INTO session_tags (id, session_id, tag_id) VALUES (?, ?, ?)")
        .bind(ST3_A)
        .bind(SESSION_Y)
        .bind(TAG1)
        .execute(&a)
        .await
        .unwrap();
    sqlx::query("INSERT INTO session_tags (id, session_id, tag_id) VALUES (?, ?, ?)")
        .bind(ST3_B)
        .bind(SESSION_Y)
        .bind(TAG1)
        .execute(&b)
        .await
        .unwrap();
    println!(
        "[both] concurrently, independently associated session {SESSION_Y} with tag {TAG1} (different row ids: {ST3_A} on A, {ST3_B} on B)"
    );

    for _ in 0..MAX_DRAIN {
        sync_and_drain(&a, &a_tcp, &a_token, "A").await;
        sync_and_drain(&b, &b_tcp, &b_token, "B").await;
    }

    let live_a = live_associations(&a, SESSION_Y, TAG1).await;
    let live_b = live_associations(&b, SESSION_Y, TAG1).await;
    assert_eq!(
        live_a, live_b,
        "live association set for (session_y, tag1) diverged A vs B"
    );
    assert_eq!(
        live_a.len(),
        2,
        "FINDING: concurrent identical add-tag actions on two offline devices converge \
         CLEANLY (both nodes agree, no torn state) but produce TWO session_tags rows for \
         the same (session_id, tag_id) pair, not one — the schema has no UNIQUE constraint \
         on that pair and the join-row PK is independently generated per device. CRDT \
         convergence does not imply application-level idempotency here."
    );
    println!(
        "[conv] concurrent identical add OK — CRDT converges cleanly to {} agreeing rows \
         (documented duplicate-association finding, see assertion message)",
        live_a.len()
    );

    // 5b. Scenario: concurrent add vs remove of the same association. Tag
    //     TAG1 is tagged on session Z via row X on both nodes; A removes it
    //     (soft-deletes X) while B, unaware, independently re-adds the same
    //     association as a fresh row Y. Net effect after sync: X stays
    //     tombstoned, Y survives — the association exists post-sync despite
    //     A's removal. This must converge identically on both sides, with no
    //     resurrection of the specific row A deleted.
    const SESSION_Z: &str = "sess-z";
    const ST4_X: &str = "77777777-7777-7777-7777-777777777777";
    sqlx::query("INSERT INTO session_tags (id, session_id, tag_id) VALUES (?, ?, ?)")
        .bind(ST4_X)
        .bind(SESSION_Z)
        .bind(TAG1)
        .execute(&a)
        .await
        .unwrap();
    run_sync(&a, &a_tcp, &a_token).await;
    drain_check(&b, &b_tcp, &b_token, "B").await;
    println!("[setup] session {SESSION_Z} tagged with {TAG1} via row {ST4_X}, synced to both");

    const ST4_Y: &str = "88888888-8888-8888-8888-888888888888";
    sqlx::query("UPDATE session_tags SET deleted_at = '2026-09-01T00:00:01Z' WHERE id = ?")
        .bind(ST4_X)
        .execute(&a)
        .await
        .unwrap();
    sqlx::query("INSERT INTO session_tags (id, session_id, tag_id) VALUES (?, ?, ?)")
        .bind(ST4_Y)
        .bind(SESSION_Z)
        .bind(TAG1)
        .execute(&b)
        .await
        .unwrap();
    println!(
        "[concurrent] A removed association {ST4_X} while B, unaware, independently re-added it as {ST4_Y}"
    );

    for _ in 0..MAX_DRAIN {
        sync_and_drain(&a, &a_tcp, &a_token, "A").await;
        sync_and_drain(&b, &b_tcp, &b_token, "B").await;
    }

    assert!(
        session_tag_row(&a, ST4_X).await.unwrap().2.is_some(),
        "removed association {ST4_X} must stay tombstoned on A"
    );
    assert!(
        session_tag_row(&b, ST4_X).await.unwrap().2.is_some(),
        "removed association {ST4_X} must stay tombstoned on B (no resurrection)"
    );
    let live_z_a = live_associations(&a, SESSION_Z, TAG1).await;
    let live_z_b = live_associations(&b, SESSION_Z, TAG1).await;
    assert_eq!(
        live_z_a, live_z_b,
        "live association set for (session_z, tag1) diverged A vs B"
    );
    assert_eq!(
        live_z_a,
        vec![ST4_Y.to_string()],
        "expected exactly the concurrently-re-added row {ST4_Y} to survive as the live association"
    );
    println!(
        "[conv] concurrent add-vs-remove OK — the removed row ({ST4_X}) stayed tombstoned on \
         both nodes, the concurrently-added row ({ST4_Y}) is the sole surviving live association \
         on both nodes"
    );

    // 6. Multi-row catch-up across both tables.
    for n in 0..3 {
        let id = format!("aaaa0000-0000-0000-0000-{n:012x}");
        sqlx::query("INSERT INTO tags (id, name) VALUES (?, ?)")
            .bind(&id)
            .bind(format!("bulk-tag-a-{n}"))
            .execute(&a)
            .await
            .unwrap();
    }
    for n in 0..3 {
        let id = format!("bbbb0000-0000-0000-0000-{n:012x}");
        sqlx::query("INSERT INTO tags (id, name) VALUES (?, ?)")
            .bind(&id)
            .bind(format!("bulk-tag-b-{n}"))
            .execute(&b)
            .await
            .unwrap();
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
        "\n=== tags + session_tags schema proof: converge; join-table duplicate-on-concurrent-add \
         is a real, documented finding (not a torn merge) ==="
    );

    a.close().await;
    b.close().await;
    agent_a.stop().await;
    agent_b.stop().await;
}

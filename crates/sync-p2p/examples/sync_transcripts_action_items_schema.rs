//! SYNC-10 (table-proofs lane): the CRDT converges notare's **real**
//! `transcripts` and `action_items` tables — the two most user-visible
//! tables in the registry that do NOT sync today (meeting transcripts and
//! the action items extracted from them). Same §17 harness and scenario
//! list as `sync_sessions_schema.rs`; the schema under test and the
//! realistic-size check are new.
//!
//! Both `CREATE TABLE` bodies below are copied **verbatim** from
//! `crates/db-app/migrations/20260710223922_canonical_data_model.sql`
//! (lines 78-97 for `transcripts`, 115-134 for `action_items`), and
//! `action_items` additionally replays the six `ALTER TABLE ... ADD COLUMN`
//! statements from `20260723130000_action_items_v2.sql` in the same order
//! production applies them, rather than hand-merging a single CREATE TABLE.
//! Neither table declares a FOREIGN KEY — matching production, and matching
//! the §19 correction that the §17 proof's FK on `session_documents` does
//! NOT exist in the real migration. `session_id` here is a plain
//! NOT-NULL-DEFAULT-'' TEXT column, unenforced, exactly as shipped.
//!
//! Run: `cargo run -p sync-p2p --example sync_transcripts_action_items_schema --features from-source`

use std::str::FromStr;
use std::time::Duration;

use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sync_p2p::{Identity, P2pAgent, PeerStore, register_direct_addr};

const DB_ID: &str = "notare-v06";
const MAX_DRAIN: usize = 16;

/// Verbatim from `20260710223922_canonical_data_model.sql:78-97`.
const CREATE_TRANSCRIPTS: &str = "CREATE TABLE IF NOT EXISTS transcripts (
  id                    TEXT PRIMARY KEY NOT NULL,
  workspace_id          TEXT NOT NULL DEFAULT '',
  owner_user_id         TEXT NOT NULL DEFAULT '',
  session_id            TEXT NOT NULL DEFAULT '',
  source                TEXT NOT NULL DEFAULT '',
  provider              TEXT NOT NULL DEFAULT '',
  model                 TEXT NOT NULL DEFAULT '',
  language              TEXT NOT NULL DEFAULT '',
  started_at_ms         INTEGER NOT NULL DEFAULT 0,
  ended_at_ms           INTEGER,
  audio_attachment_id   TEXT NOT NULL DEFAULT '',
  memo                   TEXT NOT NULL DEFAULT '',
  words_json            TEXT NOT NULL DEFAULT '[]',
  speaker_hints_json    TEXT NOT NULL DEFAULT '[]',
  metadata_json         TEXT NOT NULL DEFAULT '{}',
  created_at            TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at            TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  deleted_at            TEXT
) STRICT";

/// Verbatim from `20260710223922_canonical_data_model.sql:115-134` (the
/// base columns; the v2 columns are added below via the real ALTER
/// TABLE statements, not folded in here).
const CREATE_ACTION_ITEMS: &str = "CREATE TABLE IF NOT EXISTS action_items (
  id                 TEXT PRIMARY KEY NOT NULL,
  workspace_id       TEXT NOT NULL DEFAULT '',
  session_id         TEXT NOT NULL DEFAULT '',
  source_type        TEXT NOT NULL DEFAULT '',
  source_id          TEXT NOT NULL DEFAULT '',
  source_order       INTEGER NOT NULL DEFAULT 0,
  assignee_human_id  TEXT NOT NULL DEFAULT '',
  status             TEXT NOT NULL DEFAULT 'todo',
  text               TEXT NOT NULL DEFAULT '',
  body_json          TEXT NOT NULL DEFAULT '{}',
  due_at             TEXT NOT NULL DEFAULT '',
  completed_at       TEXT,
  created_by         TEXT NOT NULL DEFAULT '',
  updated_by         TEXT NOT NULL DEFAULT '',
  metadata_json      TEXT NOT NULL DEFAULT '{}',
  created_at         TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at         TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  deleted_at         TEXT
) STRICT";

/// Verbatim from `20260723130000_action_items_v2.sql`, applied in the same
/// order as the real migration.
const ACTION_ITEMS_V2_ALTERS: &[&str] = &[
    "ALTER TABLE action_items ADD COLUMN confidence REAL NOT NULL DEFAULT 0",
    "ALTER TABLE action_items ADD COLUMN source_text TEXT NOT NULL DEFAULT ''",
    "ALTER TABLE action_items ADD COLUMN source_start_ms INTEGER",
    "ALTER TABLE action_items ADD COLUMN owner_speaker_id TEXT NOT NULL DEFAULT ''",
    "ALTER TABLE action_items ADD COLUMN priority TEXT NOT NULL DEFAULT ''",
    "ALTER TABLE action_items ADD COLUMN synced_targets_json TEXT NOT NULL DEFAULT '[]'",
];

/// A realistic ~60-minute meeting transcript: ~9000 words, diarized between
/// two speakers, each word carrying start/end ms and a speaker tag — the
/// shape the real transcription pipeline writes to `words_json`. This is the
/// "large JSON column" case the task calls out: tiny fixtures would hide
/// CRDT overhead or blob-size problems that only show up at realistic size.
fn realistic_words_json(word_count: usize) -> String {
    let mut words = Vec::with_capacity(word_count);
    for i in 0..word_count {
        let start = i as u64 * 350;
        let end = start + 300;
        let speaker = if i % 17 < 9 { "S1" } else { "S2" };
        words.push(serde_json::json!({
            "text": format!("word{i}"),
            "start_ms": start,
            "end_ms": end,
            "speaker": speaker,
            "confidence": 0.91,
        }));
    }
    serde_json::to_string(&words).unwrap()
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

    sqlx::query(CREATE_TRANSCRIPTS)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(CREATE_ACTION_ITEMS)
        .execute(&pool)
        .await
        .unwrap();
    for alter in ACTION_ITEMS_V2_ALTERS {
        sqlx::query(*alter).execute(&pool).await.unwrap();
    }

    for table in ["transcripts", "action_items"] {
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

async fn transcript_words_len(pool: &SqlitePool, id: &str) -> Option<usize> {
    let words_json: Option<String> =
        sqlx::query_scalar("SELECT words_json FROM transcripts WHERE id = ?")
            .bind(id)
            .fetch_optional(pool)
            .await
            .unwrap();
    words_json.map(|s| s.len())
}

async fn transcript_deleted_at(pool: &SqlitePool, id: &str) -> Option<Option<String>> {
    sqlx::query_scalar("SELECT deleted_at FROM transcripts WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await
        .unwrap()
}

async fn action_item_row(pool: &SqlitePool, id: &str) -> Option<(String, String, f64, String)> {
    sqlx::query_as::<_, (String, String, f64, String)>(
        "SELECT session_id, status, confidence, priority FROM action_items WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .unwrap()
}

async fn action_item_status(pool: &SqlitePool, id: &str) -> Option<String> {
    sqlx::query_scalar("SELECT status FROM action_items WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await
        .unwrap()
}

async fn count(pool: &SqlitePool, table: &str) -> i64 {
    let sql: &'static str = match table {
        "transcripts" => "SELECT COUNT(*) FROM transcripts",
        "action_items" => "SELECT COUNT(*) FROM action_items",
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
    println!(
        "[nodes] A and B initialized; cloudsync enabled on transcripts + action_items (broker = A)"
    );

    // 1. Scenario A->B: A writes a realistic-size transcript (~9000 words,
    //    diarized) plus an action item extracted from it (with the v2
    //    provenance columns populated, as the real extractor does).
    const T1: &str = "11111111-1111-1111-1111-111111111111";
    const AI1: &str = "22222222-2222-2222-2222-222222222222";
    let big_words = realistic_words_json(9000);
    println!(
        "[A] realistic transcript payload: {} bytes",
        big_words.len()
    );
    sqlx::query(
        "INSERT INTO transcripts (id, session_id, source, provider, words_json, started_at_ms, ended_at_ms)
         VALUES (?, 'sess-a', 'mic', 'whisper', ?, 0, 3600000)",
    )
    .bind(T1)
    .bind(&big_words)
    .execute(&a)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO action_items (id, session_id, status, text, source_text, confidence, priority)
         VALUES (?, 'sess-a', 'todo', 'Follow up with client', 'word42 word43 word44', 0.87, 'high')",
    )
    .bind(AI1)
    .execute(&a)
    .await
    .unwrap();
    println!("[A] wrote transcript (realistic size) + action item");

    let sync_start = std::time::Instant::now();
    run_sync(&a, &a_tcp, &a_token).await;
    drain_check(&b, &b_tcp, &b_token, "B").await;
    let elapsed = sync_start.elapsed();
    println!("[timing] A->B sync of realistic transcript took {elapsed:?}");

    assert_eq!(
        transcript_words_len(&b, T1).await,
        Some(big_words.len()),
        "B must receive the transcript's words_json byte-for-byte (length check)"
    );
    let b_words: Option<String> =
        sqlx::query_scalar("SELECT words_json FROM transcripts WHERE id = ?")
            .bind(T1)
            .fetch_optional(&b)
            .await
            .unwrap();
    assert_eq!(
        b_words,
        Some(big_words.clone()),
        "words_json must be byte-identical on B"
    );
    assert_eq!(
        action_item_row(&b, AI1).await,
        Some(("sess-a".into(), "todo".into(), 0.87, "high".into())),
        "B has A's action item, including the v2 confidence/priority columns"
    );
    println!(
        "[conv] A -> B OK (transcripts realistic size intact, action_items v2 columns intact)"
    );

    // 2. Scenario B->A: reverse direction, smaller rows.
    const T2: &str = "33333333-3333-3333-3333-333333333333";
    const AI2: &str = "44444444-4444-4444-4444-444444444444";
    sqlx::query(
        "INSERT INTO transcripts (id, session_id, source, words_json) VALUES (?, 'sess-b', 'import', ?)",
    )
    .bind(T2)
    .bind(realistic_words_json(50))
    .execute(&b)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO action_items (id, session_id, status, text) VALUES (?, 'sess-b', 'todo', 'B item')",
    )
    .bind(AI2)
    .execute(&b)
    .await
    .unwrap();
    println!("[B] wrote transcript + action item");

    run_sync(&b, &b_tcp, &b_token).await;
    drain_check(&a, &a_tcp, &a_token, "A").await;

    assert!(
        transcript_words_len(&a, T2).await.is_some(),
        "A has B's transcript"
    );
    assert_eq!(
        action_item_status(&a, AI2).await,
        Some("todo".into()),
        "A has B's action item"
    );
    println!("[conv] B -> A OK");

    // 3. Scenario: disconnected concurrent UPDATE of the same action_items
    //    row's `status` (a realistic conflict: both devices resolve the same
    //    item while offline) converges conflict-free.
    sqlx::query("UPDATE action_items SET status = 'done' WHERE id = ?")
        .bind(AI1)
        .execute(&a)
        .await
        .unwrap();
    sqlx::query("UPDATE action_items SET status = 'cancelled' WHERE id = ?")
        .bind(AI1)
        .execute(&b)
        .await
        .unwrap();
    println!("[both] updated action item {AI1}'s status concurrently while disconnected");

    for _ in 0..MAX_DRAIN {
        sync_and_drain(&a, &a_tcp, &a_token, "A").await;
        sync_and_drain(&b, &b_tcp, &b_token, "B").await;
        let (sa, sb) = (
            action_item_status(&a, AI1).await,
            action_item_status(&b, AI1).await,
        );
        if sa == sb {
            sync_and_drain(&a, &a_tcp, &a_token, "A").await;
            sync_and_drain(&b, &b_tcp, &b_token, "B").await;
            let (fa, fb) = (
                action_item_status(&a, AI1).await,
                action_item_status(&b, AI1).await,
            );
            assert_eq!(fa, fb, "status diverged A vs B after a settling round");
            assert_eq!(
                fa, sa,
                "status was not stable — agreed on {sa:?} then moved to {fa:?}"
            );
            let settled = fa.expect("row vanished during settle");
            assert!(
                settled == "done" || settled == "cancelled",
                "converged value {settled:?} is neither of the two writes — torn or merged"
            );
            println!("[conv] concurrent update converged and held (status = {settled:?} on both)");
            break;
        }
    }

    // 4. Scenario: tombstone-as-delete. A soft-deletes the transcript
    //    (deleted_at) and hard-deletes an action item; B must see the exact
    //    deleted_at value, the action item must be gone, and neither must
    //    resurrect across further sync rounds.
    sqlx::query("UPDATE transcripts SET deleted_at = '2026-09-01T00:00:00Z' WHERE id = ?")
        .bind(T1)
        .execute(&a)
        .await
        .unwrap();
    sqlx::query("DELETE FROM action_items WHERE id = ?")
        .bind(AI2)
        .execute(&a)
        .await
        .unwrap();
    println!("[A] soft-deleted transcript {T1} and hard-deleted action item {AI2}");

    run_sync(&a, &a_tcp, &a_token).await;
    drain_check(&b, &b_tcp, &b_token, "B").await;

    assert_eq!(
        transcript_deleted_at(&b, T1).await,
        Some(Some("2026-09-01T00:00:00Z".into())),
        "the deleted_at tombstone value must sync across verbatim"
    );
    assert!(
        action_item_row(&b, AI2).await.is_none(),
        "the deleted action item must not resurrect on B"
    );

    for _ in 0..3 {
        sync_and_drain(&a, &a_tcp, &a_token, "A").await;
        sync_and_drain(&b, &b_tcp, &b_token, "B").await;
    }
    assert!(
        action_item_row(&b, AI2).await.is_none(),
        "deleted action item resurrected on B after further syncs"
    );
    assert!(
        action_item_row(&a, AI2).await.is_none(),
        "deleted action item resurrected on A after further syncs"
    );
    assert_eq!(
        transcript_deleted_at(&b, T1).await,
        Some(Some("2026-09-01T00:00:00Z".into())),
        "deleted_at tombstone changed after further syncs"
    );
    println!("[conv] tombstone-as-delete OK, no resurrection after further sync rounds");

    // 5. Scenario: multi-row catch-up across both tables.
    for n in 0..3 {
        let id = format!("aaaa0000-0000-0000-0000-{n:012x}");
        sqlx::query("INSERT INTO transcripts (id, session_id, words_json) VALUES (?, 'sess-a', ?)")
            .bind(&id)
            .bind(realistic_words_json(20))
            .execute(&a)
            .await
            .unwrap();
    }
    for n in 0..3 {
        let id = format!("bbbb0000-0000-0000-0000-{n:012x}");
        sqlx::query("INSERT INTO action_items (id, session_id, text) VALUES (?, 'sess-a', ?)")
            .bind(&id)
            .bind(format!("bulk item {n} from A"))
            .execute(&a)
            .await
            .unwrap();
    }
    for n in 0..3 {
        let id = format!("cccc0000-0000-0000-0000-{n:012x}");
        sqlx::query("INSERT INTO transcripts (id, session_id, words_json) VALUES (?, 'sess-b', ?)")
            .bind(&id)
            .bind(realistic_words_json(20))
            .execute(&b)
            .await
            .unwrap();
    }
    println!("[both] wrote bulk rows before draining");

    for _ in 0..MAX_DRAIN {
        sync_and_drain(&a, &a_tcp, &a_token, "A").await;
        sync_and_drain(&b, &b_tcp, &b_token, "B").await;
    }

    assert_eq!(
        count(&a, "transcripts").await,
        count(&b, "transcripts").await,
        "transcript count differs A vs B"
    );
    assert_eq!(
        count(&a, "action_items").await,
        count(&b, "action_items").await,
        "action_items count differs A vs B"
    );
    println!("[conv] multi-row catch-up OK (full set equality across both tables)");

    println!(
        "\n=== transcripts + action_items schema proof: converge, incl. realistic-size words_json and v2 columns ==="
    );

    a.close().await;
    b.close().await;
    agent_a.stop().await;
    agent_b.stop().await;
}

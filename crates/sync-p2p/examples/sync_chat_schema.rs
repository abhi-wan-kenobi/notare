//! SYNC-10 (table-proofs lane, batch 4): notare's **real** `chat_groups` and
//! `chat_messages` tables — the last two live tables in the registry.
//!
//! Both are written with `INSERT ... ON CONFLICT(id) DO UPDATE` and a
//! caller-supplied id minted by `id()` at
//! `apps/desktop/src/chat/store/use-chat-actions.ts:76` (`messageId`) and
//! `:88` (`currentGroupId`), with **no dedup guard** — there is no
//! find-or-create-by-title path for a chat. So a group and a message are
//! each minted on one device and replicated, the same shape as
//! `organizations` (§25) and user `templates` (§27), not the
//! locally-guarded shape that duplicates in §25/§26.
//!
//! Two cases here are specific to chat and worth more than plain row
//! convergence:
//!
//! - **Scenario 3** — both devices append a *new* message to the same group
//!   while disconnected. These are genuinely two different messages, not one
//!   entity forked, so both surviving is correct. What matters is whether the
//!   two nodes agree on the resulting transcript *order*, since a chat is
//!   read as an ordered sequence and the app orders by `created_at`.
//!
//! - **Scenario 4** — `deleteChatMessagesExcept`
//!   (`apps/desktop/src/chat/store/queries.ts:214-232`), the regenerate /
//!   edit-and-resubmit path. It bulk-tombstones every message in a group
//!   whose id is `NOT IN` a retained set:
//!
//!   ```sql
//!   UPDATE chat_messages SET deleted_at = ?, updated_at = ?
//!   WHERE chat_group_id = ? AND deleted_at IS NULL
//!     AND id NOT IN (SELECT value FROM json_each(?))
//!   ```
//!
//!   That retained set is computed from the writing device's **local** view
//!   of the conversation — the same local-view assumption behind §25's
//!   `NOT EXISTS` defect, in a different disguise. A message another device
//!   appended concurrently was never in the local view, so it is neither
//!   retained nor pruned: it simply survives the regenerate. Scenario 4
//!   measures what the merged transcript actually looks like.
//!
//! `CREATE TABLE` bodies are copied verbatim from
//! `crates/db-app/migrations/20260710223922_canonical_data_model.sql`
//! (lines 189-197 `chat_groups`, 199-212 `chat_messages`).
//!
//! Run: `cargo run -p sync-p2p --example sync_chat_schema --features from-source`

use std::str::FromStr;
use std::time::Duration;

use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sync_p2p::{Identity, P2pAgent, PeerStore, register_direct_addr};

const DB_ID: &str = "notare-v06";
const MAX_DRAIN: usize = 16;

/// Verbatim from `20260710223922_canonical_data_model.sql:189-197`.
const CREATE_CHAT_GROUPS: &str = "CREATE TABLE IF NOT EXISTS chat_groups (
  id             TEXT PRIMARY KEY NOT NULL,
  workspace_id   TEXT NOT NULL DEFAULT '',
  owner_user_id  TEXT NOT NULL DEFAULT '',
  title          TEXT NOT NULL DEFAULT '',
  created_at     TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at     TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  deleted_at     TEXT
) STRICT";

/// Verbatim from `20260710223922_canonical_data_model.sql:199-212`.
const CREATE_CHAT_MESSAGES: &str = "CREATE TABLE IF NOT EXISTS chat_messages (
  id             TEXT PRIMARY KEY NOT NULL,
  workspace_id   TEXT NOT NULL DEFAULT '',
  chat_group_id  TEXT NOT NULL DEFAULT '',
  owner_user_id  TEXT NOT NULL DEFAULT '',
  role           TEXT NOT NULL DEFAULT '',
  content        TEXT NOT NULL DEFAULT '',
  metadata_json  TEXT NOT NULL DEFAULT '{}',
  parts_json     TEXT NOT NULL DEFAULT '[]',
  status         TEXT NOT NULL DEFAULT 'ready',
  created_at     TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at     TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  deleted_at     TEXT
) STRICT";

const TABLES_UNDER_TEST: [&str; 2] = ["chat_groups", "chat_messages"];

/// Verbatim from `apps/desktop/src/chat/store/queries.ts:128-139`.
async fn upsert_chat_group(
    pool: &SqlitePool,
    group_id: &str,
    owner: &str,
    title: &str,
    created_at: &str,
    now: &str,
) {
    sqlx::query(
        "INSERT INTO chat_groups (
            id, workspace_id, owner_user_id, title, created_at, updated_at,
            deleted_at
        )
        VALUES (?, '', ?, ?, ?, ?, NULL)
        ON CONFLICT(id) DO UPDATE SET
            owner_user_id = excluded.owner_user_id,
            title = excluded.title,
            updated_at = excluded.updated_at,
            deleted_at = NULL",
    )
    .bind(group_id)
    .bind(owner)
    .bind(title)
    .bind(created_at)
    .bind(now)
    .execute(pool)
    .await
    .unwrap();
}

/// Verbatim from `buildUpsertChatMessageStatement`
/// (`apps/desktop/src/chat/store/queries.ts:249-268`).
#[allow(clippy::too_many_arguments)]
async fn upsert_chat_message(
    pool: &SqlitePool,
    id: &str,
    group_id: &str,
    owner: &str,
    role: &str,
    content: &str,
    status: &str,
    created_at: &str,
    updated_at: &str,
) {
    sqlx::query(
        "INSERT INTO chat_messages (
            id, workspace_id, chat_group_id, owner_user_id, role, content,
            metadata_json, parts_json, status, created_at, updated_at, deleted_at
        )
        VALUES (?, '', ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL)
        ON CONFLICT(id) DO UPDATE SET
            chat_group_id = excluded.chat_group_id,
            owner_user_id = excluded.owner_user_id,
            role = excluded.role,
            content = excluded.content,
            metadata_json = excluded.metadata_json,
            parts_json = excluded.parts_json,
            status = excluded.status,
            updated_at = excluded.updated_at,
            deleted_at = NULL",
    )
    .bind(id)
    .bind(group_id)
    .bind(owner)
    .bind(role)
    .bind(content)
    .bind("{}")
    .bind("[]")
    .bind(status)
    .bind(created_at)
    .bind(updated_at)
    .execute(pool)
    .await
    .unwrap();
}

/// Verbatim from `deleteChatMessagesExcept`
/// (`apps/desktop/src/chat/store/queries.ts:220-231`) — the regenerate path.
/// `retained` is serialised to a JSON array exactly as the app does.
async fn delete_chat_messages_except(
    pool: &SqlitePool,
    group_id: &str,
    retained: &[&str],
    now: &str,
) {
    let retained_json = serde_json::to_string(retained).unwrap();
    sqlx::query(
        "UPDATE chat_messages
         SET deleted_at = ?, updated_at = ?
         WHERE chat_group_id = ?
           AND deleted_at IS NULL
           AND id NOT IN (SELECT value FROM json_each(?))",
    )
    .bind(now)
    .bind(now)
    .bind(group_id)
    .bind(retained_json)
    .execute(pool)
    .await
    .unwrap();
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

    for ddl in [CREATE_CHAT_GROUPS, CREATE_CHAT_MESSAGES] {
        sqlx::query(ddl).execute(&pool).await.unwrap();
    }

    for table in TABLES_UNDER_TEST {
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

/// The live transcript of a group, in the order the app renders it
/// (`ORDER BY created_at, id`). This is the value that has to match across
/// nodes for a conversation to read the same on both devices.
async fn transcript(pool: &SqlitePool, group_id: &str) -> Vec<String> {
    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT id FROM chat_messages
         WHERE chat_group_id = ? AND deleted_at IS NULL
         ORDER BY created_at, id",
    )
    .bind(group_id)
    .fetch_all(pool)
    .await
    .unwrap();
    rows.into_iter().map(|(id,)| id).collect()
}

/// (role, content, status, updated_at, deleted_at)
async fn message_row(
    pool: &SqlitePool,
    id: &str,
) -> Option<(String, String, String, String, Option<String>)> {
    sqlx::query_as(
        "SELECT role, content, status, updated_at, deleted_at FROM chat_messages WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .unwrap()
}

/// (title, updated_at, deleted_at)
async fn group_row(pool: &SqlitePool, id: &str) -> Option<(String, String, Option<String>)> {
    sqlx::query_as("SELECT title, updated_at, deleted_at FROM chat_groups WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await
        .unwrap()
}

async fn count(pool: &SqlitePool, table: &str) -> i64 {
    let sql: &'static str = match table {
        "chat_groups" => "SELECT COUNT(*) FROM chat_groups",
        "chat_messages" => "SELECT COUNT(*) FROM chat_messages",
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
    println!("[nodes] A and B initialized; cloudsync enabled on chat_groups + chat_messages");

    const GROUP: &str = "grp-1";

    // 1. A starts a chat (group + first message, the app's real
    //    create-group-with-message transaction) and it replicates.
    upsert_chat_group(
        &a,
        GROUP,
        "user-a",
        "How do I export?",
        "2026-09-02T10:00:00.000Z",
        "2026-09-02T10:00:00.000Z",
    )
    .await;
    upsert_chat_message(
        &a,
        "msg-1",
        GROUP,
        "user-a",
        "user",
        "How do I export?",
        "ready",
        "2026-09-02T10:00:00.000Z",
        "2026-09-02T10:00:00.000Z",
    )
    .await;
    println!("[A] started chat {GROUP} with msg-1");

    run_sync(&a, &a_tcp, &a_token).await;
    drain_check(&b, &b_tcp, &b_token, "B").await;

    assert_eq!(
        group_row(&b, GROUP).await.map(|r| r.0),
        Some("How do I export?".to_string()),
        "B must receive the group"
    );
    assert_eq!(
        transcript(&b, GROUP).await,
        vec!["msg-1".to_string()],
        "B must receive the first message"
    );
    println!("[conv] chat group + message A -> B OK");

    // 2. B replies (the assistant turn, written on B) and it comes back.
    upsert_chat_message(
        &b,
        "msg-2",
        GROUP,
        "user-a",
        "assistant",
        "Use File > Export.",
        "ready",
        "2026-09-02T10:00:05.000Z",
        "2026-09-02T10:00:05.000Z",
    )
    .await;
    run_sync(&b, &b_tcp, &b_token).await;
    drain_check(&a, &a_tcp, &a_token, "A").await;
    assert_eq!(
        transcript(&a, GROUP).await,
        vec!["msg-1".to_string(), "msg-2".to_string()],
        "A must see the reply, in order"
    );
    println!("[conv] chat message B -> A OK, transcript order preserved");

    // 3. Concurrent per-column edits to the SAME message row: A rewrites the
    //    content while B flips the streaming status. Both should survive.
    let t_edit = "2026-09-02T10:01:00.000Z";
    sqlx::query("UPDATE chat_messages SET content = ?, updated_at = ? WHERE id = ?")
        .bind("Use File > Export, then pick a format.")
        .bind(t_edit)
        .bind("msg-2")
        .execute(&a)
        .await
        .unwrap();
    sqlx::query("UPDATE chat_messages SET status = ?, updated_at = ? WHERE id = ?")
        .bind("streaming")
        .bind(t_edit)
        .bind("msg-2")
        .execute(&b)
        .await
        .unwrap();
    println!("[concurrent] A rewrote msg-2's content; B changed its status");

    for _ in 0..MAX_DRAIN {
        sync_and_drain(&a, &a_tcp, &a_token, "A").await;
        sync_and_drain(&b, &b_tcp, &b_token, "B").await;
    }
    let m2_a = message_row(&a, "msg-2").await.unwrap();
    let m2_b = message_row(&b, "msg-2").await.unwrap();
    assert_eq!(
        m2_a, m2_b,
        "chat_messages row msg-2 diverged A vs B after concurrent column edits"
    );
    assert_eq!(
        (m2_a.1.as_str(), m2_a.2.as_str()),
        ("Use File > Export, then pick a format.", "streaming"),
        "per-column merge lost an edit"
    );
    println!(
        "[conv] chat_messages concurrent per-column merge OK (content and status both survived)"
    );

    // 4. Both devices append a NEW message to the same group while
    //    disconnected. These are two genuinely different messages, so both
    //    surviving is correct — the question is whether the two nodes agree
    //    on the resulting transcript ORDER, since a chat is read as a
    //    sequence and the app orders by `created_at`.
    upsert_chat_message(
        &a,
        "msg-3a",
        GROUP,
        "user-a",
        "user",
        "And to CSV?",
        "ready",
        "2026-09-02T10:02:00.000Z",
        "2026-09-02T10:02:00.000Z",
    )
    .await;
    upsert_chat_message(
        &b,
        "msg-3b",
        GROUP,
        "user-a",
        "user",
        "What about PDF?",
        "ready",
        "2026-09-02T10:02:01.000Z",
        "2026-09-02T10:02:01.000Z",
    )
    .await;
    println!("[concurrent] A appended msg-3a; B appended msg-3b to the same group");

    for _ in 0..MAX_DRAIN {
        sync_and_drain(&a, &a_tcp, &a_token, "A").await;
        sync_and_drain(&b, &b_tcp, &b_token, "B").await;
    }
    let tr_a = transcript(&a, GROUP).await;
    let tr_b = transcript(&b, GROUP).await;
    assert_eq!(
        tr_a, tr_b,
        "the two nodes render DIFFERENT transcripts for the same conversation — that is a \
         real convergence failure, not a benign interleave"
    );
    assert_eq!(
        tr_a,
        vec![
            "msg-1".to_string(),
            "msg-2".to_string(),
            "msg-3a".to_string(),
            "msg-3b".to_string()
        ],
        "both concurrent appends must survive, ordered deterministically by created_at"
    );
    println!(
        "[conv] concurrent appends both survived and BOTH nodes agree on the transcript order \
         ({tr_a:?}) — two different messages are correctly two messages, not a fork"
    );

    // 5. The regenerate path against a STALE local view. Baseline: both nodes
    //    agree on the 4-message transcript above. Then, disconnected, A
    //    appends msg-4a while B "edits and resubmits" from msg-1 — calling
    //    the app's real `deleteChatMessagesExcept(GROUP, ["msg-1"])`, whose
    //    retained set is computed from B's local view and therefore cannot
    //    mention a message B has never seen.
    upsert_chat_message(
        &a,
        "msg-4a",
        GROUP,
        "user-a",
        "assistant",
        "CSV is under the same menu.",
        "ready",
        "2026-09-02T10:03:00.000Z",
        "2026-09-02T10:03:00.000Z",
    )
    .await;
    delete_chat_messages_except(&b, GROUP, &["msg-1"], "2026-09-02T10:03:01.000Z").await;
    println!(
        "[concurrent] A appended msg-4a; B regenerated from msg-1 via the real \
         deleteChatMessagesExcept(retained=[msg-1]) — B's retained set cannot name msg-4a, \
         which B has never seen"
    );

    for _ in 0..MAX_DRAIN {
        sync_and_drain(&a, &a_tcp, &a_token, "A").await;
        sync_and_drain(&b, &b_tcp, &b_token, "B").await;
    }
    let after_a = transcript(&a, GROUP).await;
    let after_b = transcript(&b, GROUP).await;
    assert_eq!(
        after_a, after_b,
        "the nodes disagree on the transcript after the regenerate race — a genuine \
         convergence failure"
    );
    println!("[RESULT] after regenerate-vs-concurrent-append, BOTH nodes agree on: {after_a:?}");

    // Pin the exact transcript rather than inferring from its length. An
    // earlier version of this scenario asserted only convergence and then
    // reported the defect if `after_a.len() > 1`, which a *no-op* delete
    // would also satisfy — the finding could have fired for the wrong
    // reason. This assertion separates the three claims: `msg-1` was
    // retained, the messages B could actually see (`msg-2`, `msg-3a`,
    // `msg-3b`) really were tombstoned, and `msg-4a` — the one B had never
    // seen — survived a prune that was meant to discard its branch.
    assert_eq!(
        after_a,
        vec!["msg-1".to_string(), "msg-4a".to_string()],
        "expected the regenerate to prune exactly what B could see and to MISS the \
         concurrently-appended msg-4a; a different set means either the prune did not run \
         (msg-2/3a/3b would still be present) or it reached further than B's local view"
    );
    println!(
        "[FINDING] the regenerate pruned msg-2/msg-3a/msg-3b but NOT msg-4a: the retained set \
         was computed from B's local view, so a message B had never seen was neither retained \
         nor tombstoned and survived the prune. The nodes converge, but the conversation now \
         reads as the re-asked question followed by a stray answer from the branch that was \
         supposed to be discarded. Same local-view assumption as §25's NOT EXISTS defect, in \
         a different disguise. See §29.3."
    );

    // 6. Group-level soft delete converges and does not resurrect. Note the
    //    app tombstones the GROUP row only; message rows keep their own
    //    `deleted_at`, so they are not cascaded (the UI filters by group).
    let removed_at = "2026-09-02T10:04:00.000Z";
    sqlx::query("UPDATE chat_groups SET deleted_at = ?, updated_at = ? WHERE id = ?")
        .bind(removed_at)
        .bind(removed_at)
        .bind(GROUP)
        .execute(&a)
        .await
        .unwrap();
    println!("[A] soft-deleted chat group {GROUP}");

    for _ in 0..MAX_DRAIN {
        sync_and_drain(&a, &a_tcp, &a_token, "A").await;
        sync_and_drain(&b, &b_tcp, &b_token, "B").await;
    }
    assert_eq!(
        group_row(&b, GROUP).await.and_then(|r| r.2),
        Some(removed_at.to_string()),
        "the group tombstone must sync across verbatim"
    );
    for _ in 0..3 {
        sync_and_drain(&a, &a_tcp, &a_token, "A").await;
        sync_and_drain(&b, &b_tcp, &b_token, "B").await;
    }
    assert_eq!(
        group_row(&a, GROUP).await.and_then(|r| r.2),
        Some(removed_at.to_string()),
        "group tombstone changed / resurrected on A"
    );
    assert_eq!(
        group_row(&b, GROUP).await.and_then(|r| r.2),
        Some(removed_at.to_string()),
        "group tombstone changed / resurrected on B"
    );
    println!("[conv] chat_groups tombstone OK, no resurrection across further sync rounds");

    // 7. Multi-row catch-up across both tables.
    for n in 0..3 {
        upsert_chat_group(
            &a,
            &format!("grp-bulk-a-{n}"),
            "user-a",
            &format!("Bulk A {n}"),
            "2026-09-02T11:00:00.000Z",
            "2026-09-02T11:00:00.000Z",
        )
        .await;
        upsert_chat_message(
            &b,
            &format!("msg-bulk-b-{n}"),
            GROUP,
            "user-a",
            "user",
            &format!("Bulk B {n}"),
            "ready",
            "2026-09-02T11:00:00.000Z",
            "2026-09-02T11:00:00.000Z",
        )
        .await;
    }
    println!("[both] wrote 3 bulk groups (A) and 3 bulk messages (B) before draining");

    for _ in 0..MAX_DRAIN {
        sync_and_drain(&a, &a_tcp, &a_token, "A").await;
        sync_and_drain(&b, &b_tcp, &b_token, "B").await;
    }
    for table in TABLES_UNDER_TEST {
        assert_eq!(
            count(&a, table).await,
            count(&b, table).await,
            "{table} row count differs A vs B after multi-row catch-up"
        );
    }
    println!("[conv] multi-row catch-up OK (count equality on both tables)");

    println!(
        "\n=== chat schema proof: `chat_groups` and `chat_messages` CONVERGE — per-column \
         merge, deterministic transcript order under concurrent appends, and tombstones \
         without resurrection. See §29.3 for the regenerate-path caveat ==="
    );

    a.close().await;
    b.close().await;
    agent_a.stop().await;
    agent_b.stop().await;
}

//! SYNC-10 (table-proofs lane, batch 2): notare's **real** `organizations`,
//! `humans` and `session_participants` tables.
//!
//! This batch was chosen together because all three are the *contacts* family
//! and, unlike every table proven so far, two of them are written through an
//! **application-level dedup guard rather than a deterministic primary key**.
//! That is a materially different CRDT case and it is the point of this proof.
//!
//! §23.4 established the pattern for `tags`/`session_tags`: the app derives
//! those primary keys from content, so two devices performing "the same"
//! action write the SAME row and the CRDT merges them per-column. `humans`
//! and `session_participants` do the opposite — the id is
//! `crypto.randomUUID()` (`apps/desktop/src/shared/utils.ts:9`) and
//! uniqueness is enforced by a `NOT EXISTS (...)` subquery **evaluated
//! locally at write time**:
//!
//! - `humans`, calendar-participant path
//!   (`apps/desktop/src/services/calendar/storage.ts:586-609`): inserts
//!   `SELECT ... WHERE NOT EXISTS (SELECT 1 FROM humans WHERE deleted_at IS
//!   NULL AND lower(email) = lower(?))`. The human id comes from
//!   `humanId = id()` in
//!   `apps/desktop/src/services/calendar/process/participants/sync.ts:100`.
//! - `humans`, manual-contact path
//!   (`apps/desktop/src/contacts/queries.ts:273-278`): a plain INSERT with
//!   `id()`, no dedup guard at all.
//! - `session_participants` (`apps/desktop/src/session/queries.ts:434-452`
//!   and `apps/desktop/src/services/calendar/storage.ts:628-683`): inserts
//!   `id()` guarded by `NOT EXISTS (... session_id = ? AND human_id = ? AND
//!   deleted_at IS NULL)`.
//!
//! A local `NOT EXISTS` cannot see a concurrent write on another device. So
//! when two disconnected devices add the same person, or add the same person
//! to the same session, each guard passes locally, each device mints a
//! different random id, and **both rows survive the merge** — a duplicate the
//! CRDT is structurally unable to resolve, because two different primary keys
//! are two different rows, both valid.
//!
//! Scenarios 3 and 5 below prove exactly that, and they are the reason this
//! lane does **not** enable `humans` or `session_participants`. See
//! docs/internal/sync-p2p.md §25 for the verdict and the recommended fix.
//! `organizations` is a different case and does converge cleanly.
//!
//! `CREATE TABLE` bodies are copied verbatim from
//! `crates/db-app/migrations/20260710223922_canonical_data_model.sql`
//! (lines 1-13 `organizations`, 15-32 `humans`, 34-58 `sessions`, 99-113
//! `session_participants`). `sessions` is included because the real
//! `session_participants` INSERT is a `SELECT ... FROM sessions JOIN humans`
//! and cannot run without it; it is already proven and enabled (§17), and is
//! present here as a join fixture, not as a table under test.
//!
//! Run: `cargo run -p sync-p2p --example sync_contacts_schema --features from-source`

use std::str::FromStr;
use std::time::Duration;

use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sync_p2p::{Identity, P2pAgent, PeerStore, register_direct_addr};

const DB_ID: &str = "notare-v06";
const MAX_DRAIN: usize = 16;

/// Verbatim from `20260710223922_canonical_data_model.sql:1-13`.
const CREATE_ORGANIZATIONS: &str = "CREATE TABLE IF NOT EXISTS organizations (
  id             TEXT PRIMARY KEY NOT NULL,
  workspace_id   TEXT NOT NULL DEFAULT '',
  owner_user_id  TEXT NOT NULL DEFAULT '',
  name           TEXT NOT NULL DEFAULT '',
  memo           TEXT NOT NULL DEFAULT '',
  pinned         INTEGER NOT NULL DEFAULT 0,
  pin_order      INTEGER,
  metadata_json  TEXT NOT NULL DEFAULT '{}',
  created_at     TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at     TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  deleted_at     TEXT
) STRICT";

/// Verbatim from `20260710223922_canonical_data_model.sql:15-32`.
const CREATE_HUMANS: &str = "CREATE TABLE IF NOT EXISTS humans (
  id                 TEXT PRIMARY KEY NOT NULL,
  workspace_id       TEXT NOT NULL DEFAULT '',
  owner_user_id      TEXT NOT NULL DEFAULT '',
  organization_id    TEXT NOT NULL DEFAULT '',
  name               TEXT NOT NULL DEFAULT '',
  email              TEXT NOT NULL DEFAULT '',
  phone              TEXT NOT NULL DEFAULT '',
  job_title          TEXT NOT NULL DEFAULT '',
  linkedin_username  TEXT NOT NULL DEFAULT '',
  memo               TEXT NOT NULL DEFAULT '',
  pinned             INTEGER NOT NULL DEFAULT 0,
  pin_order          INTEGER,
  metadata_json      TEXT NOT NULL DEFAULT '{}',
  created_at         TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at         TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  deleted_at         TEXT
) STRICT";

/// Verbatim from `20260710223922_canonical_data_model.sql:34-58`. Join
/// fixture for the real `session_participants` INSERT, not a table under test.
const CREATE_SESSIONS: &str = "CREATE TABLE IF NOT EXISTS sessions (
  id                   TEXT PRIMARY KEY NOT NULL,
  workspace_id         TEXT NOT NULL DEFAULT '',
  owner_user_id        TEXT NOT NULL DEFAULT '',
  title                TEXT NOT NULL DEFAULT '',
  kind                 TEXT NOT NULL DEFAULT 'meeting',
  status               TEXT NOT NULL DEFAULT 'active',
  created_at           TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at           TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  started_at           TEXT NOT NULL DEFAULT '',
  ended_at             TEXT NOT NULL DEFAULT '',
  timezone             TEXT NOT NULL DEFAULT '',
  language             TEXT NOT NULL DEFAULT '',
  event_id             TEXT NOT NULL DEFAULT '',
  external_event_id    TEXT NOT NULL DEFAULT '',
  external_provider    TEXT NOT NULL DEFAULT '',
  series_id            TEXT NOT NULL DEFAULT '',
  source_apps_json     TEXT NOT NULL DEFAULT '[]',
  event_json           TEXT NOT NULL DEFAULT '',
  folder_path          TEXT NOT NULL DEFAULT '',
  slug                 TEXT NOT NULL DEFAULT '',
  metadata_json        TEXT NOT NULL DEFAULT '{}',
  deleted_at           TEXT
) STRICT";

/// Verbatim from `20260710223922_canonical_data_model.sql:99-113`.
const CREATE_SESSION_PARTICIPANTS: &str = "CREATE TABLE IF NOT EXISTS session_participants (
  id             TEXT PRIMARY KEY NOT NULL,
  workspace_id   TEXT NOT NULL DEFAULT '',
  owner_user_id  TEXT NOT NULL DEFAULT '',
  session_id     TEXT NOT NULL DEFAULT '',
  human_id       TEXT NOT NULL DEFAULT '',
  display_name   TEXT NOT NULL DEFAULT '',
  email          TEXT NOT NULL DEFAULT '',
  role           TEXT NOT NULL DEFAULT '',
  source         TEXT NOT NULL DEFAULT '',
  metadata_json  TEXT NOT NULL DEFAULT '{}',
  created_at     TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at     TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  deleted_at     TEXT
) STRICT";

const TABLES_UNDER_TEST: [&str; 4] = [
    "organizations",
    "humans",
    "session_participants",
    "sessions",
];

/// Verbatim from `apps/desktop/src/contacts/queries.ts:300-307` — the real
/// "create organization" INSERT. `id` is caller-supplied (`id()` in the app).
async fn create_organization(pool: &SqlitePool, org_id: &str, name: &str, owner: &str, now: &str) {
    sqlx::query(
        "INSERT INTO organizations (
            id, workspace_id, owner_user_id, name, memo, pinned, pin_order,
            metadata_json, created_at, updated_at, deleted_at
        ) VALUES (?, '', ?, ?, '', 0, NULL, '{}', ?, ?, NULL)",
    )
    .bind(org_id)
    .bind(owner)
    .bind(name)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await
    .unwrap();
}

/// Verbatim from `apps/desktop/src/contacts/queries.ts:273-278` — the real
/// manual "create contact" INSERT. No dedup guard of any kind.
async fn create_human_manual(
    pool: &SqlitePool,
    human_id: &str,
    name: &str,
    email: &str,
    owner: &str,
    now: &str,
) {
    sqlx::query(
        "INSERT INTO humans (
            id, workspace_id, owner_user_id, organization_id, name, email,
            phone, job_title, linkedin_username, memo, pinned, pin_order,
            metadata_json, created_at, updated_at, deleted_at
        ) VALUES (?, '', ?, '', ?, ?, '', '', '', '', 0, NULL, '{}', ?, ?, NULL)",
    )
    .bind(human_id)
    .bind(owner)
    .bind(name)
    .bind(email)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await
    .unwrap();
}

/// Verbatim from `apps/desktop/src/services/calendar/storage.ts:586-609` —
/// the real calendar-participant "create human" INSERT, including its
/// `NOT EXISTS ... lower(email)` guard. This is the write pattern whose
/// dedup is local-only.
async fn create_human_via_calendar(
    pool: &SqlitePool,
    human_id: &str,
    name: &str,
    email: &str,
    owner: &str,
    now: &str,
) {
    sqlx::query(
        "INSERT INTO humans (
            id,
            owner_user_id,
            name,
            email,
            created_at,
            updated_at,
            deleted_at
        )
        SELECT ?, ?, ?, ?, ?, ?, NULL
        WHERE NOT EXISTS (
            SELECT 1
            FROM humans
            WHERE deleted_at IS NULL AND lower(email) = lower(?)
        )",
    )
    .bind(human_id)
    .bind(owner)
    .bind(name)
    .bind(email)
    .bind(now)
    .bind(now)
    .bind(email)
    .execute(pool)
    .await
    .unwrap();
}

/// Verbatim from `apps/desktop/src/session/queries.ts:434-452` — the real
/// "add participant to session" INSERT, including its local
/// `NOT EXISTS (session_id, human_id, deleted_at IS NULL)` guard.
async fn add_session_participant(
    pool: &SqlitePool,
    participant_id: &str,
    session_id: &str,
    human_id: &str,
    source: &str,
    now: &str,
) {
    sqlx::query(
        "INSERT INTO session_participants (
            id, workspace_id, owner_user_id, session_id, human_id,
            display_name, email, role, source, metadata_json, created_at,
            updated_at, deleted_at
        )
        SELECT ?, '', session.owner_user_id, session.id, human.id,
            human.name, human.email, '', ?, '{}', ?, ?, NULL
        FROM sessions AS session
        JOIN humans AS human ON human.id = ? AND human.deleted_at IS NULL
        WHERE session.id = ?
            AND session.deleted_at IS NULL
            AND NOT EXISTS (
                SELECT 1
                FROM session_participants AS existing
                WHERE existing.session_id = session.id
                    AND existing.human_id = human.id
                    AND existing.deleted_at IS NULL
            )",
    )
    .bind(participant_id)
    .bind(source)
    .bind(now)
    .bind(now)
    .bind(human_id)
    .bind(session_id)
    .execute(pool)
    .await
    .unwrap();
}

async fn insert_session(pool: &SqlitePool, session_id: &str, title: &str, owner: &str, now: &str) {
    sqlx::query(
        "INSERT INTO sessions (id, owner_user_id, title, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?)
         ON CONFLICT(id) DO UPDATE SET title = excluded.title",
    )
    .bind(session_id)
    .bind(owner)
    .bind(title)
    .bind(now)
    .bind(now)
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

    for ddl in [
        CREATE_ORGANIZATIONS,
        CREATE_HUMANS,
        CREATE_SESSIONS,
        CREATE_SESSION_PARTICIPANTS,
    ] {
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

/// (name, memo, owner_user_id, updated_at, deleted_at)
async fn org_row(
    pool: &SqlitePool,
    id: &str,
) -> Option<(String, String, String, String, Option<String>)> {
    sqlx::query_as(
        "SELECT name, memo, owner_user_id, updated_at, deleted_at FROM organizations WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .unwrap()
}

/// Live (non-tombstoned) human ids for an email — what a contacts UI
/// deduplicating by email would have to reconcile.
async fn live_humans_by_email(pool: &SqlitePool, email: &str) -> Vec<String> {
    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT id FROM humans WHERE lower(email) = lower(?) AND deleted_at IS NULL ORDER BY id",
    )
    .bind(email)
    .fetch_all(pool)
    .await
    .unwrap();
    rows.into_iter().map(|(id,)| id).collect()
}

/// Live participant rows for a (session, human) pair — what a participant
/// list would render.
async fn live_participants(pool: &SqlitePool, session_id: &str, human_id: &str) -> Vec<String> {
    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT id FROM session_participants
         WHERE session_id = ? AND human_id = ? AND deleted_at IS NULL ORDER BY id",
    )
    .bind(session_id)
    .bind(human_id)
    .fetch_all(pool)
    .await
    .unwrap();
    rows.into_iter().map(|(id,)| id).collect()
}

/// (session_id, human_id, display_name, source, updated_at, deleted_at)
async fn participant_row(
    pool: &SqlitePool,
    id: &str,
) -> Option<(String, String, String, String, String, Option<String>)> {
    sqlx::query_as(
        "SELECT session_id, human_id, display_name, source, updated_at, deleted_at
         FROM session_participants WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .unwrap()
}

async fn count(pool: &SqlitePool, table: &str) -> i64 {
    let sql: &'static str = match table {
        "organizations" => "SELECT COUNT(*) FROM organizations",
        "humans" => "SELECT COUNT(*) FROM humans",
        "session_participants" => "SELECT COUNT(*) FROM session_participants",
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
        "[nodes] A and B initialized; cloudsync enabled on organizations + humans + \
         session_participants (+ sessions as a join fixture; broker = A)"
    );

    let t0 = "2026-09-02T00:00:00.000Z";

    // 1. Scenario A->B and B->A: `organizations`, the ordinary case. A row is
    //    created on one device with a random id and replicated; there is no
    //    concurrent-creation ambiguity because only one device minted the id.
    create_organization(&a, "org-acme", "Acme Corp", "user-a", t0).await;
    println!("[A] created organization org-acme 'Acme Corp'");

    run_sync(&a, &a_tcp, &a_token).await;
    drain_check(&b, &b_tcp, &b_token, "B").await;

    let org_b = org_row(&b, "org-acme")
        .await
        .expect("B has A's organization");
    assert_eq!(
        (org_b.0.as_str(), org_b.4.clone()),
        ("Acme Corp", None),
        "B's organization row"
    );
    println!("[conv] organizations A -> B OK");

    create_organization(&b, "org-globex", "Globex", "user-b", t0).await;
    run_sync(&b, &b_tcp, &b_token).await;
    drain_check(&a, &a_tcp, &a_token, "A").await;
    let org_a = org_row(&a, "org-globex")
        .await
        .expect("A has B's organization");
    assert_eq!(org_a.0, "Globex", "A's organization name");
    println!("[conv] organizations B -> A OK");

    // 2. Scenario: concurrent edits to DIFFERENT columns of the SAME
    //    organization row. Both devices already have org-acme, disconnect,
    //    then A renames it while B writes a memo. CLS merges per column, so
    //    both edits should survive — neither should clobber the other.
    let t_rename = "2026-09-02T00:01:00.000Z";
    sqlx::query("UPDATE organizations SET name = ?, updated_at = ? WHERE id = ?")
        .bind("Acme Corporation")
        .bind(t_rename)
        .bind("org-acme")
        .execute(&a)
        .await
        .unwrap();
    sqlx::query("UPDATE organizations SET memo = ?, updated_at = ? WHERE id = ?")
        .bind("preferred vendor")
        .bind(t_rename)
        .bind("org-acme")
        .execute(&b)
        .await
        .unwrap();
    println!("[concurrent] A renamed org-acme; B wrote its memo (different columns, same row)");

    for _ in 0..MAX_DRAIN {
        sync_and_drain(&a, &a_tcp, &a_token, "A").await;
        sync_and_drain(&b, &b_tcp, &b_token, "B").await;
    }

    let merged_a = org_row(&a, "org-acme").await.unwrap();
    let merged_b = org_row(&b, "org-acme").await.unwrap();
    assert_eq!(
        merged_a, merged_b,
        "organizations row org-acme diverged A vs B after the concurrent column edits"
    );
    assert_eq!(
        (merged_a.0.as_str(), merged_a.1.as_str()),
        ("Acme Corporation", "preferred vendor"),
        "per-column merge lost an edit: expected BOTH the rename and the memo to survive"
    );
    println!(
        "[conv] organizations concurrent per-column merge OK (name={:?}, memo={:?} — both edits survived)",
        merged_a.0, merged_a.1
    );

    // 3. THE FINDING for `humans`. Two disconnected devices independently add
    //    the SAME person, by email, through the real calendar-participant
    //    write path — exactly what happens when the same meeting invite is
    //    processed on a laptop and a desktop that are both offline. Each
    //    device's `NOT EXISTS ... lower(email)` guard passes locally, because
    //    neither can see the other's row, and each mints its own
    //    `crypto.randomUUID()`. After the merge both rows exist.
    const SHARED_EMAIL: &str = "alice@example.com";
    let human_a = "human-uuid-from-device-a";
    let human_b = "human-uuid-from-device-b";
    create_human_via_calendar(&a, human_a, "Alice", SHARED_EMAIL, "user-a", t0).await;
    create_human_via_calendar(&b, human_b, "Alice", SHARED_EMAIL, "user-b", t0).await;
    assert_eq!(
        live_humans_by_email(&a, SHARED_EMAIL).await,
        vec![human_a.to_string()],
        "precondition: A must have exactly its own row before syncing"
    );
    assert_eq!(
        live_humans_by_email(&b, SHARED_EMAIL).await,
        vec![human_b.to_string()],
        "precondition: B must have exactly its own row before syncing"
    );
    println!(
        "[concurrent] A and B each added {SHARED_EMAIL:?} while disconnected, via the real \
         calendar path's NOT EXISTS(lower(email)) guard — different random ids ({human_a}, \
         {human_b})"
    );

    for _ in 0..MAX_DRAIN {
        sync_and_drain(&a, &a_tcp, &a_token, "A").await;
        sync_and_drain(&b, &b_tcp, &b_token, "B").await;
    }

    let dupes_a = live_humans_by_email(&a, SHARED_EMAIL).await;
    let dupes_b = live_humans_by_email(&b, SHARED_EMAIL).await;
    assert_eq!(
        dupes_a, dupes_b,
        "the two nodes do not even agree on the duplicate set — that would be a convergence \
         failure on top of the duplication"
    );
    // Both rows are present, and the CRDT is structurally unable to remove
    // either: two distinct primary keys are two distinct rows, each internally
    // consistent. This assertion encodes the DEFECT so that making the id
    // deterministic (the recommended fix) breaks this test and forces §25 to
    // be revisited, rather than leaving the hazard undetected.
    assert_eq!(
        dupes_a.len(),
        2,
        "expected the local-only email guard to produce a DUPLICATE pair after merge; got {dupes_a:?}"
    );
    assert!(
        dupes_a.contains(&human_a.to_string()) && dupes_a.contains(&human_b.to_string()),
        "both devices' rows should be the duplicate pair, got {dupes_a:?}"
    );
    println!(
        "[FINDING] humans DID NOT dedup: {SHARED_EMAIL:?} exists TWICE after merge on both \
         nodes ({dupes_a:?}). The rows CONVERGE (both nodes agree) but the entity is \
         DUPLICATED — a local NOT EXISTS guard cannot see a concurrent remote insert, and CLS \
         cannot merge two different primary keys. `humans` must NOT be enabled on this write \
         pattern; see docs/internal/sync-p2p.md §25."
    );

    // For completeness: the same hazard exists on the manual-contact path,
    // which has no guard at all — so it is strictly worse, not better.
    let manual_a = "human-manual-a";
    let manual_b = "human-manual-b";
    create_human_manual(&a, manual_a, "Bob", "bob@example.com", "user-a", t0).await;
    create_human_manual(&b, manual_b, "Bob", "bob@example.com", "user-b", t0).await;
    for _ in 0..MAX_DRAIN {
        sync_and_drain(&a, &a_tcp, &a_token, "A").await;
        sync_and_drain(&b, &b_tcp, &b_token, "B").await;
    }
    assert_eq!(
        live_humans_by_email(&a, "bob@example.com").await.len(),
        2,
        "the unguarded manual-contact path must also duplicate"
    );
    println!(
        "[FINDING] humans manual-contact path (no guard at all, contacts/queries.ts:273) \
         duplicates identically"
    );

    // 4. Scenario: a `humans` row created on ONE device and then edited
    //    concurrently converges cleanly — the duplication above is a
    //    *creation* hazard, not a merge failure, and this separates the two.
    let solo = "human-solo";
    create_human_manual(&a, solo, "Carol", "carol@example.com", "user-a", t0).await;
    run_sync(&a, &a_tcp, &a_token).await;
    drain_check(&b, &b_tcp, &b_token, "B").await;
    let t_edit = "2026-09-02T00:02:00.000Z";
    sqlx::query("UPDATE humans SET job_title = ?, updated_at = ? WHERE id = ?")
        .bind("CTO")
        .bind(t_edit)
        .bind(solo)
        .execute(&a)
        .await
        .unwrap();
    sqlx::query("UPDATE humans SET phone = ?, updated_at = ? WHERE id = ?")
        .bind("+1-555-0100")
        .bind(t_edit)
        .bind(solo)
        .execute(&b)
        .await
        .unwrap();
    for _ in 0..MAX_DRAIN {
        sync_and_drain(&a, &a_tcp, &a_token, "A").await;
        sync_and_drain(&b, &b_tcp, &b_token, "B").await;
    }
    let solo_a: (String, String) =
        sqlx::query_as("SELECT job_title, phone FROM humans WHERE id = ?")
            .bind(solo)
            .fetch_one(&a)
            .await
            .unwrap();
    let solo_b: (String, String) =
        sqlx::query_as("SELECT job_title, phone FROM humans WHERE id = ?")
            .bind(solo)
            .fetch_one(&b)
            .await
            .unwrap();
    assert_eq!(solo_a, solo_b, "humans row {solo} diverged A vs B");
    assert_eq!(
        (solo_a.0.as_str(), solo_a.1.as_str()),
        ("CTO", "+1-555-0100"),
        "per-column merge lost an edit on a singly-created humans row"
    );
    println!(
        "[conv] humans per-column merge on a singly-created row OK (job_title={:?}, phone={:?}) \
         — the defect in scenario 3 is duplicate CREATION, not a merge failure",
        solo_a.0, solo_a.1
    );

    // 5. THE FINDING for `session_participants`. Same shape: both devices add
    //    the same person to the same session while disconnected, through the
    //    real INSERT with its local NOT EXISTS(session_id, human_id) guard.
    const SESSION_M: &str = "sess-m";
    insert_session(&a, SESSION_M, "Weekly sync", "user-a", t0).await;
    create_human_manual(&a, "human-dave", "Dave", "dave@example.com", "user-a", t0).await;
    run_sync(&a, &a_tcp, &a_token).await;
    drain_check(&b, &b_tcp, &b_token, "B").await;
    println!("[setup] session {SESSION_M} + human-dave created on A and synced to both");

    let part_a = "participant-uuid-a";
    let part_b = "participant-uuid-b";
    add_session_participant(&a, part_a, SESSION_M, "human-dave", "manual", t0).await;
    add_session_participant(&b, part_b, SESSION_M, "human-dave", "manual", t0).await;
    assert_eq!(
        live_participants(&a, SESSION_M, "human-dave").await,
        vec![part_a.to_string()],
        "precondition: A has exactly its own participant row before syncing"
    );
    assert_eq!(
        live_participants(&b, SESSION_M, "human-dave").await,
        vec![part_b.to_string()],
        "precondition: B has exactly its own participant row before syncing"
    );
    println!(
        "[concurrent] A and B each added human-dave to {SESSION_M} while disconnected, via the \
         real guarded INSERT — different random ids ({part_a}, {part_b})"
    );

    for _ in 0..MAX_DRAIN {
        sync_and_drain(&a, &a_tcp, &a_token, "A").await;
        sync_and_drain(&b, &b_tcp, &b_token, "B").await;
    }

    let plist_a = live_participants(&a, SESSION_M, "human-dave").await;
    let plist_b = live_participants(&b, SESSION_M, "human-dave").await;
    assert_eq!(
        plist_a, plist_b,
        "the nodes disagree on the participant duplicate set — convergence failure on top of \
         duplication"
    );
    assert_eq!(
        plist_a.len(),
        2,
        "expected the local-only participant guard to produce a DUPLICATE pair after merge; \
         got {plist_a:?}"
    );
    println!(
        "[FINDING] session_participants DID NOT dedup: human-dave appears TWICE in \
         {SESSION_M} after merge on both nodes ({plist_a:?}) — the same local-guard hazard. \
         Must NOT be enabled on this write pattern; see §25."
    );

    // 6. Scenario: participant soft-delete (the app's real
    //    `removeSessionParticipant`) converges and does not resurrect. The
    //    duplication hazard does not extend to removal — a tombstone on a
    //    known row id behaves like every other table's.
    let removed_at = "2026-09-02T00:03:00.000Z";
    sqlx::query("UPDATE session_participants SET deleted_at = ?, updated_at = ? WHERE id = ?")
        .bind(removed_at)
        .bind(removed_at)
        .bind(part_a)
        .execute(&a)
        .await
        .unwrap();
    println!("[A] soft-deleted participant row {part_a}");

    for _ in 0..MAX_DRAIN {
        sync_and_drain(&a, &a_tcp, &a_token, "A").await;
        sync_and_drain(&b, &b_tcp, &b_token, "B").await;
    }
    assert_eq!(
        participant_row(&b, part_a).await.and_then(|r| r.5),
        Some(removed_at.to_string()),
        "the participant tombstone must sync across verbatim"
    );
    for _ in 0..3 {
        sync_and_drain(&a, &a_tcp, &a_token, "A").await;
        sync_and_drain(&b, &b_tcp, &b_token, "B").await;
    }
    assert_eq!(
        participant_row(&a, part_a).await.and_then(|r| r.5),
        Some(removed_at.to_string()),
        "participant tombstone changed / row resurrected on A"
    );
    assert_eq!(
        participant_row(&b, part_a).await.and_then(|r| r.5),
        Some(removed_at.to_string()),
        "participant tombstone changed / row resurrected on B"
    );
    // Removing one of the duplicate pair leaves exactly one live row, which is
    // also the shape any dedup repair would produce.
    assert_eq!(
        live_participants(&a, SESSION_M, "human-dave").await,
        vec![part_b.to_string()],
        "after tombstoning one duplicate, exactly the other should remain live"
    );
    println!(
        "[conv] session_participants tombstone OK, no resurrection across further sync rounds"
    );

    // 7. Multi-row catch-up across all three tables under test.
    for n in 0..3 {
        create_organization(
            &a,
            &format!("org-bulk-a-{n}"),
            &format!("Bulk A {n}"),
            "user-a",
            t0,
        )
        .await;
        create_human_manual(
            &b,
            &format!("human-bulk-b-{n}"),
            &format!("Bulk B {n}"),
            &format!("bulk-b-{n}@example.com"),
            "user-b",
            t0,
        )
        .await;
    }
    println!("[both] wrote 3 bulk organizations (A) and 3 bulk humans (B) before draining");

    for _ in 0..MAX_DRAIN {
        sync_and_drain(&a, &a_tcp, &a_token, "A").await;
        sync_and_drain(&b, &b_tcp, &b_token, "B").await;
    }
    for table in ["organizations", "humans", "session_participants"] {
        assert_eq!(
            count(&a, table).await,
            count(&b, table).await,
            "{table} row count differs A vs B after multi-row catch-up"
        );
    }
    println!("[conv] multi-row catch-up OK (full count equality across all three tables)");

    println!(
        "\n=== contacts schema proof: `organizations` CONVERGES and is safe to enable. \
         `humans` and `session_participants` converge as ROWS but DUPLICATE as ENTITIES under \
         concurrent offline creation, because both are written with a random id behind a \
         local-only NOT EXISTS guard — NO-GO for enabling; see docs/internal/sync-p2p.md §25 ==="
    );

    a.close().await;
    b.close().await;
    agent_a.stop().await;
    agent_b.stop().await;
}

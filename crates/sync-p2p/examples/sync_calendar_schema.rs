//! SYNC-10 (table-proofs lane, batch 3a): notare's **real** `calendars` and
//! `events` tables — and why neither can be enabled for P2P sync.
//!
//! These two are the same class of defect §25 found in `humans` /
//! `session_participants`, but they will fire *more* reliably, because the
//! duplicating write is not a user action — it is the calendar poller, which
//! runs automatically on every device.
//!
//! Both tables are a **local cache of provider state**, keyed by a provider
//! id held in a *non-primary* column:
//!
//! - `calendars.tracking_id_calendar` — the provider's calendar id
//! - `events.tracking_id_event` — the provider's event id
//!
//! The primary key is a locally-minted `crypto.randomUUID()`
//! (`apps/desktop/src/shared/utils.ts:9`). From
//! `apps/desktop/src/services/calendar/storage.ts`:
//!
//! - `:222` — `const calendarId = stored?.id ?? id()`, where `stored` is a
//!   lookup in the **local** `existingByTrackingKey` map.
//! - `:509` — `const eventId = id()`, for every event in `events.toAdd`,
//!   which is itself computed by diffing the provider's feed against
//!   **local** rows keyed by `eventKey(calendarId, tracking_id_event)`.
//!
//! So each device independently assigns its own primary key to the same
//! provider object. Two devices polling the same Google/Outlook account —
//! the normal case for a user with a laptop and a desktop — mint different
//! ids for the same calendar and the same event. After the CRDT merge both
//! rows survive, because two distinct primary keys are two distinct rows and
//! CLS has nothing to merge.
//!
//! Scenario 2 proves it. The verdict is NO-GO for both; see
//! docs/internal/sync-p2p.md §26. Note the deeper point recorded there:
//! this data is already replicated by the calendar provider, so syncing it
//! device-to-device duplicates a replication path that already exists,
//! rather than adding one that is missing.
//!
//! `CREATE TABLE` bodies are copied verbatim from
//! `crates/db-app/migrations/20260414120000_calendars_events.sql`, plus the
//! two `ALTER TABLE ... ADD COLUMN deleted_at TEXT` statements from
//! `20260711000000_calendar_event_tombstones.sql`, replayed in migration
//! order rather than hand-merged (the §23.2 convention). Neither table is
//! `STRICT` and both use second-precision `%SZ` timestamps, unlike the
//! canonical-data-model tables — copied as-is, not normalised.
//!
//! Run: `cargo run -p sync-p2p --example sync_calendar_schema --features from-source`

use std::str::FromStr;
use std::time::Duration;

use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sync_p2p::{Identity, P2pAgent, PeerStore, register_direct_addr};

const DB_ID: &str = "notare-v06";
const MAX_DRAIN: usize = 16;

/// Verbatim from `20260414120000_calendars_events.sql:1-12`.
const CREATE_CALENDARS: &str = "CREATE TABLE IF NOT EXISTS calendars (
  id                    TEXT PRIMARY KEY NOT NULL,
  tracking_id_calendar  TEXT NOT NULL DEFAULT '',
  name                  TEXT NOT NULL DEFAULT '',
  enabled               INTEGER NOT NULL DEFAULT 0,
  provider              TEXT NOT NULL DEFAULT '',
  source                TEXT NOT NULL DEFAULT '',
  color                 TEXT NOT NULL DEFAULT '#888',
  connection_id         TEXT NOT NULL DEFAULT '',
  created_at            TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
  updated_at            TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
)";

/// Verbatim from `20260414120000_calendars_events.sql:14-32`.
const CREATE_EVENTS: &str = "CREATE TABLE IF NOT EXISTS events (
  id                    TEXT PRIMARY KEY NOT NULL,
  tracking_id_event     TEXT NOT NULL DEFAULT '',
  calendar_id           TEXT NOT NULL DEFAULT '',
  title                 TEXT NOT NULL DEFAULT '',
  started_at            TEXT NOT NULL DEFAULT '',
  ended_at              TEXT NOT NULL DEFAULT '',
  location              TEXT NOT NULL DEFAULT '',
  meeting_link          TEXT NOT NULL DEFAULT '',
  description           TEXT NOT NULL DEFAULT '',
  note                  TEXT NOT NULL DEFAULT '',
  recurrence_series_id  TEXT NOT NULL DEFAULT '',
  has_recurrence_rules  INTEGER NOT NULL DEFAULT 0,
  is_all_day            INTEGER NOT NULL DEFAULT 0,
  provider              TEXT NOT NULL DEFAULT '',
  participants_json     TEXT,
  created_at            TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
  updated_at            TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
)";

/// Verbatim from `20260711000000_calendar_event_tombstones.sql`.
const ALTER_CALENDARS_TOMBSTONE: &str = "ALTER TABLE calendars ADD COLUMN deleted_at TEXT";
const ALTER_EVENTS_TOMBSTONE: &str = "ALTER TABLE events ADD COLUMN deleted_at TEXT";

const TABLES_UNDER_TEST: [&str; 2] = ["calendars", "events"];

/// Verbatim from `apps/desktop/src/services/calendar/storage.ts:223-250` —
/// the real calendar-ingest upsert. `calendar_id` is the caller's, which in
/// the app is `stored?.id ?? id()`: reused if this provider calendar is
/// already known **locally**, freshly random if not.
async fn ingest_calendar(
    pool: &SqlitePool,
    calendar_id: &str,
    tracking_id: &str,
    name: &str,
    provider: &str,
    now: &str,
) {
    sqlx::query(
        "INSERT INTO calendars (
            id,
            tracking_id_calendar,
            name,
            enabled,
            provider,
            source,
            color,
            connection_id,
            created_at,
            updated_at,
            deleted_at
        )
        VALUES (?, ?, ?, 0, ?, ?, ?, ?, ?, ?, NULL)
        ON CONFLICT(id) DO UPDATE SET
            tracking_id_calendar = excluded.tracking_id_calendar,
            name = excluded.name,
            enabled = CASE
                WHEN calendars.deleted_at IS NULL THEN calendars.enabled
                ELSE 0
            END,
            provider = excluded.provider,
            source = excluded.source,
            color = excluded.color,
            connection_id = excluded.connection_id,
            updated_at = excluded.updated_at,
            deleted_at = NULL",
    )
    .bind(calendar_id)
    .bind(tracking_id)
    .bind(name)
    .bind(provider)
    .bind("remote")
    .bind("#888")
    .bind("conn-1")
    .bind(now)
    .bind(now)
    .execute(pool)
    .await
    .unwrap();
}

/// Verbatim from `apps/desktop/src/services/calendar/storage.ts:511-533` —
/// the real event-ingest INSERT. In the app `id` is always a fresh
/// `id()` for anything in `events.toAdd`.
async fn ingest_event(
    pool: &SqlitePool,
    event_id: &str,
    tracking_id: &str,
    calendar_id: &str,
    title: &str,
    started_at: &str,
    now: &str,
) {
    sqlx::query(
        "INSERT INTO events (
            id,
            tracking_id_event,
            calendar_id,
            title,
            started_at,
            ended_at,
            location,
            meeting_link,
            description,
            recurrence_series_id,
            has_recurrence_rules,
            is_all_day,
            provider,
            participants_json,
            created_at,
            updated_at,
            deleted_at
        )
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL)",
    )
    .bind(event_id)
    .bind(tracking_id)
    .bind(calendar_id)
    .bind(title)
    .bind(started_at)
    .bind("")
    .bind("")
    .bind("")
    .bind("")
    .bind("")
    .bind(0)
    .bind(0)
    .bind("google")
    .bind(Option::<String>::None)
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

    // Replayed in migration order, per §23.2: create, then ALTER, rather
    // than a hand-merged CREATE TABLE that production never runs.
    for ddl in [
        CREATE_CALENDARS,
        CREATE_EVENTS,
        ALTER_CALENDARS_TOMBSTONE,
        ALTER_EVENTS_TOMBSTONE,
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

/// Live calendar ids for a provider tracking id — what "which calendars do I
/// have" resolves to. The provider id is the real identity; the PK is not.
async fn live_calendars_by_tracking(pool: &SqlitePool, tracking_id: &str) -> Vec<String> {
    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT id FROM calendars
         WHERE tracking_id_calendar = ? AND deleted_at IS NULL ORDER BY id",
    )
    .bind(tracking_id)
    .fetch_all(pool)
    .await
    .unwrap();
    rows.into_iter().map(|(id,)| id).collect()
}

/// Live event ids for a provider tracking id — what an agenda view renders.
async fn live_events_by_tracking(pool: &SqlitePool, tracking_id: &str) -> Vec<String> {
    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT id FROM events
         WHERE tracking_id_event = ? AND deleted_at IS NULL ORDER BY id",
    )
    .bind(tracking_id)
    .fetch_all(pool)
    .await
    .unwrap();
    rows.into_iter().map(|(id,)| id).collect()
}

/// (title, started_at, location, updated_at, deleted_at)
async fn event_row(
    pool: &SqlitePool,
    id: &str,
) -> Option<(String, String, String, String, Option<String>)> {
    sqlx::query_as(
        "SELECT title, started_at, location, updated_at, deleted_at FROM events WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .unwrap()
}

async fn count(pool: &SqlitePool, table: &str) -> i64 {
    let sql: &'static str = match table {
        "calendars" => "SELECT COUNT(*) FROM calendars",
        "events" => "SELECT COUNT(*) FROM events",
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
    println!("[nodes] A and B initialized; cloudsync enabled on calendars + events (broker = A)");

    let t0 = "2026-09-02T00:00:00Z";

    // 1. Baseline: only ONE device ingests, then replicates. This is the case
    //    that works, and it is the control for scenario 2 — it shows the rows
    //    themselves replicate correctly, so the defect below is about id
    //    provenance and nothing else.
    ingest_calendar(&a, "cal-local-a", "provider-cal-work", "Work", "google", t0).await;
    ingest_event(
        &a,
        "evt-local-a",
        "provider-evt-standup",
        "cal-local-a",
        "Standup",
        "2026-09-03T09:00:00Z",
        t0,
    )
    .await;
    println!("[A] ingested provider-cal-work + provider-evt-standup (only A polled)");

    run_sync(&a, &a_tcp, &a_token).await;
    drain_check(&b, &b_tcp, &b_token, "B").await;

    assert_eq!(
        live_calendars_by_tracking(&b, "provider-cal-work").await,
        vec!["cal-local-a".to_string()],
        "B must receive A's calendar row"
    );
    let evt_b = event_row(&b, "evt-local-a")
        .await
        .expect("B must receive A's event row");
    assert_eq!(
        (evt_b.0.as_str(), evt_b.1.as_str()),
        ("Standup", "2026-09-03T09:00:00Z"),
        "B's event row"
    );
    println!("[conv] single-poller ingest A -> B OK (rows replicate correctly)");

    // 2. THE FINDING. Both devices poll the SAME provider account while
    //    disconnected from each other — the normal state of a laptop and a
    //    desktop both signed into the same Google account. Each runs the real
    //    ingest path, each finds nothing locally for that tracking id, and so
    //    each mints its own `id()`.
    const TRACK_CAL: &str = "provider-cal-personal";
    const TRACK_EVT: &str = "provider-evt-review";
    ingest_calendar(&a, "cal-uuid-a", TRACK_CAL, "Personal", "google", t0).await;
    ingest_event(
        &a,
        "evt-uuid-a",
        TRACK_EVT,
        "cal-uuid-a",
        "Design Review",
        "2026-09-04T14:00:00Z",
        t0,
    )
    .await;
    ingest_calendar(&b, "cal-uuid-b", TRACK_CAL, "Personal", "google", t0).await;
    ingest_event(
        &b,
        "evt-uuid-b",
        TRACK_EVT,
        "cal-uuid-b",
        "Design Review",
        "2026-09-04T14:00:00Z",
        t0,
    )
    .await;
    assert_eq!(
        live_calendars_by_tracking(&a, TRACK_CAL).await,
        vec!["cal-uuid-a".to_string()],
        "precondition: A has only its own calendar row before syncing"
    );
    assert_eq!(
        live_calendars_by_tracking(&b, TRACK_CAL).await,
        vec!["cal-uuid-b".to_string()],
        "precondition: B has only its own calendar row before syncing"
    );
    println!(
        "[concurrent] A and B each polled the SAME provider account while disconnected: \
         {TRACK_CAL} -> (cal-uuid-a | cal-uuid-b), {TRACK_EVT} -> (evt-uuid-a | evt-uuid-b)"
    );

    for _ in 0..MAX_DRAIN {
        sync_and_drain(&a, &a_tcp, &a_token, "A").await;
        sync_and_drain(&b, &b_tcp, &b_token, "B").await;
    }

    let cals_a = live_calendars_by_tracking(&a, TRACK_CAL).await;
    let cals_b = live_calendars_by_tracking(&b, TRACK_CAL).await;
    let evts_a = live_events_by_tracking(&a, TRACK_EVT).await;
    let evts_b = live_events_by_tracking(&b, TRACK_EVT).await;
    assert_eq!(
        cals_a, cals_b,
        "the nodes disagree on the duplicate calendar set — that would be a convergence \
         failure on top of the duplication"
    );
    assert_eq!(
        evts_a, evts_b,
        "the nodes disagree on the duplicate event set"
    );
    // Encodes the DEFECT. Deriving the PK from the provider tracking id (the
    // fix in §26.3) breaks these two assertions and forces the verdict to be
    // revisited rather than silently changed.
    assert_eq!(
        cals_a.len(),
        2,
        "expected ONE provider calendar to become TWO rows after merge; got {cals_a:?}"
    );
    assert_eq!(
        evts_a.len(),
        2,
        "expected ONE provider event to become TWO rows after merge; got {evts_a:?}"
    );
    println!(
        "[FINDING] calendars DID NOT dedup: provider calendar {TRACK_CAL:?} exists TWICE after \
         merge on both nodes ({cals_a:?})"
    );
    println!(
        "[FINDING] events DID NOT dedup: provider event {TRACK_EVT:?} exists TWICE after merge \
         on both nodes ({evts_a:?}) — the user would see the same meeting twice in their \
         agenda. `calendars`/`events` must NOT be enabled; see §26."
    );

    // 3. Isolate the cause, exactly as §25 scenario 4 did: a row created on
    //    ONE device and edited concurrently on both merges correctly per
    //    column. So the defect is duplicate INGEST, not a merge failure.
    let t_edit = "2026-09-02T00:01:00Z";
    sqlx::query("UPDATE events SET title = ?, updated_at = ? WHERE id = ?")
        .bind("Standup (moved)")
        .bind(t_edit)
        .bind("evt-local-a")
        .execute(&a)
        .await
        .unwrap();
    sqlx::query("UPDATE events SET location = ?, updated_at = ? WHERE id = ?")
        .bind("Room 4")
        .bind(t_edit)
        .bind("evt-local-a")
        .execute(&b)
        .await
        .unwrap();
    println!("[concurrent] A retitled evt-local-a; B set its location (different columns)");

    for _ in 0..MAX_DRAIN {
        sync_and_drain(&a, &a_tcp, &a_token, "A").await;
        sync_and_drain(&b, &b_tcp, &b_token, "B").await;
    }
    let merged_a = event_row(&a, "evt-local-a").await.unwrap();
    let merged_b = event_row(&b, "evt-local-a").await.unwrap();
    assert_eq!(
        merged_a, merged_b,
        "events row evt-local-a diverged A vs B after concurrent column edits"
    );
    assert_eq!(
        (merged_a.0.as_str(), merged_a.2.as_str()),
        ("Standup (moved)", "Room 4"),
        "per-column merge lost an edit: both the retitle and the location should survive"
    );
    println!(
        "[conv] events concurrent per-column merge OK (title={:?}, location={:?}) — the defect \
         in scenario 2 is duplicate INGEST, not a merge failure",
        merged_a.0, merged_a.2
    );

    // 4. Tombstone convergence, using the column the tombstones migration
    //    added. A "unsubscribes" from a calendar; B must see it and it must
    //    not resurrect across further rounds.
    let removed_at = "2026-09-02T00:02:00Z";
    sqlx::query("UPDATE calendars SET deleted_at = ?, updated_at = ? WHERE id = ?")
        .bind(removed_at)
        .bind(removed_at)
        .bind("cal-local-a")
        .execute(&a)
        .await
        .unwrap();
    println!("[A] soft-deleted calendar cal-local-a");

    for _ in 0..MAX_DRAIN {
        sync_and_drain(&a, &a_tcp, &a_token, "A").await;
        sync_and_drain(&b, &b_tcp, &b_token, "B").await;
    }
    assert!(
        live_calendars_by_tracking(&b, "provider-cal-work")
            .await
            .is_empty(),
        "B must see the calendar tombstone"
    );
    for _ in 0..3 {
        sync_and_drain(&a, &a_tcp, &a_token, "A").await;
        sync_and_drain(&b, &b_tcp, &b_token, "B").await;
    }
    assert!(
        live_calendars_by_tracking(&a, "provider-cal-work")
            .await
            .is_empty()
            && live_calendars_by_tracking(&b, "provider-cal-work")
                .await
                .is_empty(),
        "calendar tombstone resurrected after further sync rounds"
    );
    println!("[conv] calendars tombstone OK, no resurrection across further sync rounds");

    // 5. Multi-row catch-up.
    for n in 0..3 {
        ingest_event(
            &a,
            &format!("evt-bulk-a-{n}"),
            &format!("provider-evt-bulk-a-{n}"),
            "cal-uuid-a",
            &format!("Bulk A {n}"),
            "2026-09-05T10:00:00Z",
            t0,
        )
        .await;
        ingest_event(
            &b,
            &format!("evt-bulk-b-{n}"),
            &format!("provider-evt-bulk-b-{n}"),
            "cal-uuid-b",
            &format!("Bulk B {n}"),
            "2026-09-05T11:00:00Z",
            t0,
        )
        .await;
    }
    println!("[both] ingested 3 bulk events each before draining");

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
    println!("[conv] multi-row catch-up OK (full count equality on both tables)");

    println!(
        "\n=== calendar schema proof: `calendars` and `events` converge as ROWS but DUPLICATE \
         every provider object under the normal two-device polling case, because the primary \
         key is a locally-minted UUID while the real identity lives in tracking_id_* — NO-GO \
         for both; see docs/internal/sync-p2p.md §26 ==="
    );

    a.close().await;
    b.close().await;
    agent_a.stop().await;
    agent_b.stop().await;
}

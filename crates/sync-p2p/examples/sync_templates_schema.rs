//! SYNC-10 (table-proofs lane, batch 3b): notare's **real** `templates`
//! table — and the first table in this suite with a genuine hard `DELETE`.
//!
//! Every table proven so far soft-deletes: they carry a `deleted_at` column
//! and "removal" is an UPDATE, so CLS resolves it by ordinary per-column
//! LWW. §23.7 recorded that as a limitation and named the missing case
//! explicitly — *"this table's soft-delete-via-column convention only gets
//! ordinary per-column LWW, not the causal-length no-resurrection guarantee
//! real `DELETE`s get elsewhere in this suite... give it a real removal path
//! (a SQL `DELETE`) once one exists in the app."*
//!
//! `templates` is that table. It has **no `deleted_at` column at all**
//! (`crates/db-app/migrations/20260413020000_templates.sql`), and removal is
//! `DELETE FROM templates WHERE id = ?`
//! (`crates/db-app/src/template_ops.rs:75`). Scenarios 4 and 5 below are the
//! first exercise of real-DELETE convergence in this suite.
//!
//! `templates` is also the counter-example to §25/§26's duplication defect,
//! and it is worth being precise about why:
//!
//! - The **built-in** templates are seeded by
//!   `20260524000000_default_templates.sql` with **fixed, content-derived
//!   ids** (`default-board-meeting`, `default-daily-standup`, ...) via
//!   `INSERT OR IGNORE`. Every device that runs migrations produces the SAME
//!   primary keys, so independent seeding converges to one row per template
//!   instead of forking — the `tags`-style deterministic-id case, arrived at
//!   by a different route.
//! - **User** templates go through `upsert_template`
//!   (`template_ops.rs:18-40`), an `INSERT ... ON CONFLICT(id) DO UPDATE`
//!   with a caller-supplied id, and the app has no find-or-create-by-title
//!   path — so a user template is minted on one device and replicated, the
//!   same shape as `organizations` in §25.
//!
//! Scenario 5 is the one with product consequences: a **fresh device** runs
//! migrations and re-seeds every default template by fixed id. If the user
//! had already deleted one on their old device, does the new device's
//! re-seed resurrect it? That is not a hypothetical — it is what happens the
//! first time someone pairs a new laptop.
//!
//! `CREATE TABLE` body copied verbatim from
//! `20260413020000_templates.sql`; note it is not `STRICT` and uses
//! second-precision `%SZ` timestamps, copied as-is. The
//! `20260712170000_template_icons.sql` ALTER is replayed after it in
//! migration order, per the §23.2 convention.
//!
//! Run: `cargo run -p sync-p2p --example sync_templates_schema --features from-source`

use std::str::FromStr;
use std::time::Duration;

use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sync_p2p::{Identity, P2pAgent, PeerStore, register_direct_addr};

const DB_ID: &str = "notare-v06";
const MAX_DRAIN: usize = 16;

/// Verbatim from `20260413020000_templates.sql`.
const CREATE_TEMPLATES: &str = "CREATE TABLE IF NOT EXISTS templates (
  id            TEXT PRIMARY KEY NOT NULL,
  title         TEXT NOT NULL DEFAULT '',
  description   TEXT NOT NULL DEFAULT '',
  pinned        INTEGER NOT NULL DEFAULT 0,
  pin_order     INTEGER,
  category      TEXT,
  targets_json  TEXT,
  sections_json TEXT NOT NULL DEFAULT '[]',
  created_at    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
  updated_at    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
)";

/// Verbatim from `20260712170000_template_icons.sql`, replayed in order
/// rather than merged into the CREATE TABLE above. Note it is `NOT NULL`
/// **with** a DEFAULT, which is what
/// `registered_tables_match_cloudsync_schema_requirements` requires of every
/// non-PK NOT NULL column — so the ALTER is load-bearing for compatibility,
/// not cosmetic.
const ALTER_TEMPLATE_ICON: &str = "ALTER TABLE templates
ADD COLUMN icon_json TEXT NOT NULL DEFAULT '{\"type\":\"icon\",\"value\":\"notebook-tabs\",\"color\":\"#9ca3af\"}'";

/// Verbatim from `crates/db-app/src/template_ops.rs:18-40` — the real
/// template upsert. Caller supplies `id`; the app has no
/// find-or-create-by-title path.
async fn upsert_template(
    pool: &SqlitePool,
    id: &str,
    title: &str,
    description: &str,
    category: Option<&str>,
    sections_json: &str,
) {
    sqlx::query(
        "INSERT INTO templates \
         (id, title, description, pinned, pin_order, category, targets_json, sections_json, updated_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, strftime('%Y-%m-%dT%H:%M:%SZ', 'now')) \
         ON CONFLICT(id) DO UPDATE SET \
           title = excluded.title, \
           description = excluded.description, \
           pinned = excluded.pinned, \
           pin_order = excluded.pin_order, \
           category = excluded.category, \
           targets_json = excluded.targets_json, \
           sections_json = excluded.sections_json, \
           updated_at = excluded.updated_at",
    )
    .bind(id)
    .bind(title)
    .bind(description)
    .bind(0)
    .bind(Option::<i64>::None)
    .bind(category)
    .bind(Option::<String>::None)
    .bind(sections_json)
    .execute(pool)
    .await
    .unwrap();
}

/// Verbatim from `crates/db-app/src/template_ops.rs:75` — a real hard
/// DELETE, not a tombstone. `templates` has no `deleted_at` column.
async fn delete_template(pool: &SqlitePool, id: &str) {
    sqlx::query("DELETE FROM templates WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await
        .unwrap();
}

/// The shape of `20260524000000_default_templates.sql`: fixed ids, seeded
/// with `INSERT OR IGNORE`, which is what EVERY device runs on first launch.
/// Two rows is enough to prove the property; the real migration has ~20.
async fn seed_default_templates(pool: &SqlitePool) {
    for (id, title, category) in [
        ("default-daily-standup", "Daily Standup", "Engineering"),
        ("default-board-meeting", "Board Meeting", "Leadership"),
    ] {
        sqlx::query(
            "INSERT OR IGNORE INTO templates (
                id, title, description, pinned, pin_order, category, targets_json, sections_json
            ) VALUES (?, ?, '', 0, NULL, ?, '[]', '[]')",
        )
        .bind(id)
        .bind(title)
        .bind(category)
        .execute(pool)
        .await
        .unwrap();
    }
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

    for ddl in [CREATE_TEMPLATES, ALTER_TEMPLATE_ICON] {
        sqlx::query(ddl).execute(&pool).await.unwrap();
    }

    sqlx::query("SELECT cloudsync_init('templates', 'cls', 1)")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("SELECT cloudsync_enable('templates')")
        .execute(&pool)
        .await
        .unwrap();
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

/// (title, description, category, pinned, updated_at)
async fn template_row(
    pool: &SqlitePool,
    id: &str,
) -> Option<(String, String, Option<String>, i64, String)> {
    sqlx::query_as(
        "SELECT title, description, category, pinned, updated_at FROM templates WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .unwrap()
}

async fn exists(pool: &SqlitePool, id: &str) -> bool {
    template_row(pool, id).await.is_some()
}

async fn ids_like(pool: &SqlitePool, prefix: &str) -> Vec<String> {
    let rows: Vec<(String,)> =
        sqlx::query_as("SELECT id FROM templates WHERE id LIKE ? ORDER BY id")
            .bind(format!("{prefix}%"))
            .fetch_all(pool)
            .await
            .unwrap();
    rows.into_iter().map(|(id,)| id).collect()
}

async fn count(pool: &SqlitePool) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM templates")
        .fetch_one(pool)
        .await
        .unwrap()
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
    println!("[nodes] A and B initialized; cloudsync enabled on templates (broker = A)");

    // 1. Fixed-id default seeding on BOTH devices independently — what two
    //    fresh installs do before ever pairing. This is the contrast case to
    //    §26's calendars/events: identical ids, so the merge produces ONE row
    //    per template rather than forking each one.
    seed_default_templates(&a).await;
    seed_default_templates(&b).await;
    println!("[both] independently seeded the fixed-id default templates (INSERT OR IGNORE)");

    for _ in 0..MAX_DRAIN {
        sync_and_drain(&a, &a_tcp, &a_token, "A").await;
        sync_and_drain(&b, &b_tcp, &b_token, "B").await;
    }

    let defaults_a = ids_like(&a, "default-").await;
    let defaults_b = ids_like(&b, "default-").await;
    assert_eq!(
        defaults_a, defaults_b,
        "the two nodes disagree on the default-template set"
    );
    assert_eq!(
        defaults_a,
        vec![
            "default-board-meeting".to_string(),
            "default-daily-standup".to_string()
        ],
        "independent seeding must converge to exactly ONE row per fixed id, not a duplicate \
         pair — this is the property calendars/events lack (§26)"
    );
    println!(
        "[conv] independent default seeding converged to ONE row per template ({defaults_a:?}) \
         — fixed ids make concurrent creation safe"
    );

    // 2. A user template created on one device replicates. No
    //    find-or-create-by-title path exists in the app, so there is no
    //    concurrent-creation ambiguity (the §25 `organizations` shape).
    upsert_template(
        &a,
        "tpl-user-1",
        "Retro",
        "Sprint retrospective",
        Some("Engineering"),
        r#"[{"title":"What went well"}]"#,
    )
    .await;
    println!("[A] created user template tpl-user-1 'Retro'");

    run_sync(&a, &a_tcp, &a_token).await;
    drain_check(&b, &b_tcp, &b_token, "B").await;

    let user_b = template_row(&b, "tpl-user-1")
        .await
        .expect("B must receive A's user template");
    assert_eq!(user_b.0, "Retro", "B's template title");
    println!("[conv] user template A -> B OK");

    // 3. Concurrent edits to DIFFERENT columns of the same template row.
    upsert_template(
        &a,
        "tpl-user-1",
        "Sprint Retro",
        "Sprint retrospective",
        Some("Engineering"),
        r#"[{"title":"What went well"}]"#,
    )
    .await;
    sqlx::query("UPDATE templates SET description = ? WHERE id = ?")
        .bind("Retrospective for the sprint")
        .bind("tpl-user-1")
        .execute(&b)
        .await
        .unwrap();
    println!("[concurrent] A retitled tpl-user-1; B rewrote its description");

    for _ in 0..MAX_DRAIN {
        sync_and_drain(&a, &a_tcp, &a_token, "A").await;
        sync_and_drain(&b, &b_tcp, &b_token, "B").await;
    }
    let merged_a = template_row(&a, "tpl-user-1").await.unwrap();
    let merged_b = template_row(&b, "tpl-user-1").await.unwrap();
    assert_eq!(
        merged_a, merged_b,
        "templates row tpl-user-1 diverged A vs B after concurrent column edits"
    );
    assert_eq!(
        (merged_a.0.as_str(), merged_a.1.as_str()),
        ("Sprint Retro", "Retrospective for the sprint"),
        "per-column merge lost an edit"
    );
    println!(
        "[conv] templates concurrent per-column merge OK (title={:?}, description={:?})",
        merged_a.0, merged_a.1
    );

    // 4. THE NEW CASE (§23.7's named gap): a real hard `DELETE` — not a
    //    tombstone UPDATE — must propagate and must not resurrect. This is
    //    the first exercise of real-DELETE convergence in this suite.
    upsert_template(&a, "tpl-doomed", "Doomed", "", None, "[]").await;
    run_sync(&a, &a_tcp, &a_token).await;
    drain_check(&b, &b_tcp, &b_token, "B").await;
    assert!(
        exists(&b, "tpl-doomed").await,
        "precondition: B must have the row before A deletes it"
    );

    delete_template(&a, "tpl-doomed").await;
    println!("[A] hard-DELETEd tpl-doomed (real DELETE, no deleted_at column exists)");

    for _ in 0..MAX_DRAIN {
        sync_and_drain(&a, &a_tcp, &a_token, "A").await;
        sync_and_drain(&b, &b_tcp, &b_token, "B").await;
    }
    assert!(
        !exists(&b, "tpl-doomed").await,
        "a real DELETE must propagate to B, not just vanish locally"
    );
    for _ in 0..3 {
        sync_and_drain(&a, &a_tcp, &a_token, "A").await;
        sync_and_drain(&b, &b_tcp, &b_token, "B").await;
    }
    assert!(
        !exists(&a, "tpl-doomed").await && !exists(&b, "tpl-doomed").await,
        "the hard-DELETEd row resurrected after further sync rounds"
    );
    println!("[conv] real hard DELETE propagated and stayed deleted on both nodes (§23.7's gap)");

    // 5. The scenario with product consequences: a real DELETE racing a
    //    FRESH DEVICE's default-template re-seed. B replays the migration's
    //    `INSERT OR IGNORE` for a fixed id that A has already deleted —
    //    exactly what happens the first time a new laptop is paired. If the
    //    re-seed wins, users get deleted templates back on every new device.
    assert!(
        exists(&a, "default-daily-standup").await,
        "precondition: the default template must exist on A before deleting it"
    );
    delete_template(&a, "default-daily-standup").await;
    for _ in 0..MAX_DRAIN {
        sync_and_drain(&a, &a_tcp, &a_token, "A").await;
        sync_and_drain(&b, &b_tcp, &b_token, "B").await;
    }
    assert!(
        !exists(&b, "default-daily-standup").await,
        "the default-template deletion must reach B first, or this isn't testing the re-seed race"
    );
    println!("[A] deleted default-daily-standup; both nodes agree it is gone");

    // Now B re-seeds, as a fresh install's migration would.
    seed_default_templates(&b).await;
    let reseeded_locally = exists(&b, "default-daily-standup").await;
    println!(
        "[B] replayed the default-template seed (INSERT OR IGNORE), as a fresh device's \
         migration does — locally present again: {reseeded_locally}"
    );

    for _ in 0..MAX_DRAIN {
        sync_and_drain(&a, &a_tcp, &a_token, "A").await;
        sync_and_drain(&b, &b_tcp, &b_token, "B").await;
    }
    let resurrect_a = exists(&a, "default-daily-standup").await;
    let resurrect_b = exists(&b, "default-daily-standup").await;
    assert_eq!(
        resurrect_a, resurrect_b,
        "the nodes disagree on whether the re-seeded default template exists — a genuine \
         convergence failure, worse than either outcome on its own"
    );
    println!(
        "[RESULT] re-seed vs prior hard DELETE converged to present={resurrect_a} on BOTH \
         nodes (see §27.4 — this is the behaviour a fresh-device pairing will show)"
    );

    // 6. Multi-row catch-up.
    for n in 0..3 {
        upsert_template(
            &a,
            &format!("tpl-bulk-a-{n}"),
            &format!("Bulk A {n}"),
            "",
            None,
            "[]",
        )
        .await;
        upsert_template(
            &b,
            &format!("tpl-bulk-b-{n}"),
            &format!("Bulk B {n}"),
            "",
            None,
            "[]",
        )
        .await;
    }
    println!("[both] wrote 3 bulk templates each before draining");

    for _ in 0..MAX_DRAIN {
        sync_and_drain(&a, &a_tcp, &a_token, "A").await;
        sync_and_drain(&b, &b_tcp, &b_token, "B").await;
    }
    assert_eq!(
        count(&a).await,
        count(&b).await,
        "templates row count differs A vs B after multi-row catch-up"
    );
    println!("[conv] multi-row catch-up OK (count equality)");

    println!(
        "\n=== templates schema proof: CONVERGES — fixed-id default seeding does not duplicate, \
         per-column merge holds, and a real hard DELETE propagates without resurrection (the \
         §23.7 gap, now closed). See §27 for the fresh-device re-seed result ==="
    );

    a.close().await;
    b.close().await;
    agent_a.stop().await;
    agent_b.stop().await;
}

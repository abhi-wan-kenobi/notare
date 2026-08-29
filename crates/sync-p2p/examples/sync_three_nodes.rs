//! SYNC-4 convergence proof: **three** independent sqlite databases converge
//! over the iroh P2P transport through an **elected hub**, with no SQLite
//! Cloud / Postgres / Supabase server.
//!
//! This is the N-way extension of `sync_two_nodes`. The v0.6 plan's topology
//! (A) is elected-hub P2P: one peer hosts the broker, the rest reach it over
//! iroh. Two nodes never exercise the interesting part — with a single spoke
//! the hub's per-site delivery log can never be more than one blob ahead of
//! anyone. With two spokes it can, which is what this proof pins down:
//!
//!   - each site gets a **separate** high-water mark in the hub's blob log, so
//!     B's writes reach C and C's reach B even though neither ever dials the
//!     other (all traffic goes spoke -> hub -> spoke);
//!   - a site that is several blobs behind catches all of them up, not just
//!     the oldest — the hub's `check` serves one blob per call, so callers
//!     must drain (see [`drain_check`]);
//!   - a concurrent update on all three sites converges to a single value.
//!
//! Run: `cargo run -p sync-p2p --example sync_three_nodes --features from-source`
//!
//! The `from-source` feature is required (and is why this is not built by a
//! plain `cargo test --workspace`): the custom P2P network layer only exists in
//! the from-source cloudsync build, and that build is linux/x86_64 only so far.

use std::str::FromStr;
use std::time::Duration;

use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sync_p2p::{Identity, P2pAgent, PeerStore, register_direct_addr};

const DB_ID: &str = "notare-v06";

/// Bound on drain/settle loops so a non-converging run fails fast and loudly
/// rather than spinning.
const MAX_DRAIN: usize = 16;

async fn setup_node(uri: &str, broker_addr: &str, local_agent_tcp: &str) -> SqlitePool {
    let options = SqliteConnectOptions::from_str(uri).unwrap();
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

    sqlx::query("CREATE TABLE IF NOT EXISTS notes (id INTEGER PRIMARY KEY, body TEXT)")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("SELECT cloudsync_init('notes', 'cls', 1)")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("SELECT cloudsync_enable('notes')")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("SELECT cloudsync_network_init_custom(?, ?)")
        .bind(broker_addr)
        .bind(DB_ID)
        .execute(&pool)
        .await
        .unwrap();

    // SAFETY: sequential single-process example; every sync call re-sets this
    // immediately before use, and no other thread reads it concurrently.
    unsafe {
        std::env::set_var("NOTARE_SYNC_AGENT_ADDR", local_agent_tcp);
    }

    pool
}

async fn count_notes(pool: &SqlitePool) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM notes")
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn note_body(pool: &SqlitePool, id: i64) -> String {
    sqlx::query_scalar("SELECT body FROM notes WHERE id = ?")
        .bind(id)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn run_sync(pool: &SqlitePool, local_agent_tcp: &str) -> String {
    // SAFETY: see setup_node.
    unsafe {
        std::env::set_var("NOTARE_SYNC_AGENT_ADDR", local_agent_tcp);
    }
    sqlx::query_scalar::<_, String>("SELECT cloudsync_network_sync()")
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn run_check(pool: &SqlitePool, local_agent_tcp: &str) -> String {
    // SAFETY: see setup_node.
    unsafe {
        std::env::set_var("NOTARE_SYNC_AGENT_ADDR", local_agent_tcp);
    }
    sqlx::query_scalar::<_, String>("SELECT cloudsync_network_check_changes()")
        .fetch_one(pool)
        .await
        .unwrap()
}

/// Pull until the hub has nothing left for this site.
///
/// The hub serves **one** blob per `check` (it walks its append-only blob log
/// from the site's high-water mark and returns the next unseen entry). A spoke
/// that missed several uploads therefore needs several checks. Returns the
/// number of checks that actually applied rows.
///
/// Parsed with serde, not string-matched. `check`'s reply is
/// `{"receive":{"rows":N,"tables":[...]}}`, and the §12 audit already called out
/// `strstr`-style JSON handling in the C layer as a real defect — a reference
/// shape SYNC-5 is meant to copy should not repeat it.
///
/// Returns `None` for a reply we could not read at all. That is deliberately
/// **not** folded into `Some(0)`: "the hub has nothing for me" and "I could not
/// tell what the hub said" are different states, and conflating them is the
/// silent-divergence bug this whole file exists to warn about. The caller must
/// decide, and here it fails loudly.
fn rows_received(resp: &str) -> Option<u64> {
    let v: serde_json::Value = serde_json::from_str(resp).ok()?;
    v.get("receive")?.get("rows")?.as_u64()
}

async fn drain_check(pool: &SqlitePool, tcp: &str, label: &str) -> usize {
    let mut applied = 0;
    for _ in 0..MAX_DRAIN {
        let resp = run_check(pool, tcp).await;
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
    // Falling out of the loop means we stopped because of the bound, not
    // because the hub was empty — the site may still be behind. Silently
    // returning here would be the same class of bug as swallowing a malformed
    // reply.
    panic!("[{label}] hit MAX_DRAIN ({MAX_DRAIN}) with changes still pending");
}

/// Push local changes, then drain everything pending for this site.
async fn sync_and_drain(pool: &SqlitePool, tcp: &str, label: &str) {
    run_sync(pool, tcp).await;
    drain_check(pool, tcp, label).await;
}

fn short(agent: &P2pAgent) -> String {
    agent.node_id().to_z32().chars().take(8).collect()
}

#[tokio::main(flavor = "multi_thread", worker_threads = 4)]
async fn main() {
    let tmp = tempfile::tempdir().unwrap();

    let dir_a = tempfile::tempdir_in(tmp.path()).unwrap();
    let dir_b = tempfile::tempdir_in(tmp.path()).unwrap();
    let dir_c = tempfile::tempdir_in(tmp.path()).unwrap();

    let id_a = Identity::load_or_create_in(dir_a.path()).unwrap();
    let id_b = Identity::load_or_create_in(dir_b.path()).unwrap();
    let id_c = Identity::load_or_create_in(dir_c.path()).unwrap();

    let peers_a = PeerStore::load_or_create_in(dir_a.path()).unwrap();
    let peers_b = PeerStore::load_or_create_in(dir_b.path()).unwrap();
    let peers_c = PeerStore::load_or_create_in(dir_c.path()).unwrap();

    // Hub-and-spoke pairing: the hub allowlists both spokes (accept side) and
    // each spoke allowlists the hub (dial side). B and C deliberately do NOT
    // allowlist each other — proving their changes reach one another purely
    // via the hub, with no spoke-to-spoke connection.
    peers_a.add_peer(id_b.id(), "Node B").unwrap();
    peers_a.add_peer(id_c.id(), "Node C").unwrap();
    peers_b.add_peer(id_a.id(), "Node A (hub)").unwrap();
    peers_c.add_peer(id_a.id(), "Node A (hub)").unwrap();

    let agent_a = P2pAgent::start_with(id_a, peers_a).await.unwrap();
    let agent_b = P2pAgent::start_with(id_b, peers_b).await.unwrap();
    let agent_c = P2pAgent::start_with(id_c, peers_c).await.unwrap();

    register_direct_addr(agent_a.node_id(), agent_a.direct_addresses()).await;
    register_direct_addr(agent_b.node_id(), agent_b.direct_addresses()).await;
    register_direct_addr(agent_c.node_id(), agent_c.direct_addresses()).await;
    tokio::time::sleep(Duration::from_millis(150)).await;

    let a_tcp = agent_a.local_addr.clone();
    let b_tcp = agent_b.local_addr.clone();
    let c_tcp = agent_c.local_addr.clone();
    let hub = agent_a.address();

    println!("[agents] A={} (hub)", short(&agent_a));
    println!("[agents] B={}", short(&agent_b));
    println!("[agents] C={}", short(&agent_c));
    println!("[peers]  hub allowlists B+C; B and C allowlist only the hub");

    let a = setup_node(
        &format!(
            "sqlite://{}?mode=rwc",
            dir_a.path().join("node_a.db").display()
        ),
        &hub,
        &a_tcp,
    )
    .await;
    let b = setup_node(
        &format!(
            "sqlite://{}?mode=rwc",
            dir_b.path().join("node_b.db").display()
        ),
        &hub,
        &b_tcp,
    )
    .await;
    let c = setup_node(
        &format!(
            "sqlite://{}?mode=rwc",
            dir_c.path().join("node_c.db").display()
        ),
        &hub,
        &c_tcp,
    )
    .await;
    println!("[nodes] A, B, C initialized; cloudsync enabled on 'notes' (hub = A)");

    // 1. Hub writes; both spokes must receive it.
    sqlx::query("INSERT INTO notes (id, body) VALUES (1, 'from A')")
        .execute(&a)
        .await
        .unwrap();
    run_sync(&a, &a_tcp).await;
    drain_check(&b, &b_tcp, "B").await;
    drain_check(&c, &c_tcp, "C").await;
    assert_eq!(count_notes(&b).await, 1, "B has the hub's row");
    assert_eq!(count_notes(&c).await, 1, "C has the hub's row");
    println!("[conv] A -> B and A -> C OK");

    // 2. Spoke-to-spoke through the hub. B writes; C must get it without ever
    //    talking to B. This is the case two nodes cannot exercise.
    sqlx::query("INSERT INTO notes (id, body) VALUES (2, 'from B')")
        .execute(&b)
        .await
        .unwrap();
    run_sync(&b, &b_tcp).await;
    drain_check(&a, &a_tcp, "A").await;
    drain_check(&c, &c_tcp, "C").await;
    assert_eq!(count_notes(&c).await, 2, "C got B's row via the hub");
    assert_eq!(note_body(&c, 2).await, "from B");
    println!("[conv] B -> hub -> C OK (no spoke-to-spoke connection)");

    // 3. The multi-blob-behind case: C writes twice while A and B are idle, so
    //    each of them is two blobs behind and a single check cannot catch up.
    sqlx::query("INSERT INTO notes (id, body) VALUES (3, 'from C #1')")
        .execute(&c)
        .await
        .unwrap();
    run_sync(&c, &c_tcp).await;
    sqlx::query("INSERT INTO notes (id, body) VALUES (4, 'from C #2')")
        .execute(&c)
        .await
        .unwrap();
    run_sync(&c, &c_tcp).await;

    drain_check(&a, &a_tcp, "A").await;
    drain_check(&b, &b_tcp, "B").await;
    assert_eq!(
        count_notes(&a).await,
        4,
        "hub caught up on both of C's blobs"
    );
    assert_eq!(count_notes(&b).await, 4, "B caught up on both of C's blobs");
    println!("[conv] multi-blob catch-up OK (2 blobs behind -> drained)");

    // 3b. The same, spoke-to-spoke and in the other direction: B writes two
    //     blobs and C must catch up on both, having never talked to B. Step 2
    //     proved spoke->hub->spoke at depth 1 and step 3 proved depth 2 into
    //     the hub; this is the combination — depth 2, spoke to spoke.
    sqlx::query("INSERT INTO notes (id, body) VALUES (5, 'from B #1')")
        .execute(&b)
        .await
        .unwrap();
    run_sync(&b, &b_tcp).await;
    sqlx::query("INSERT INTO notes (id, body) VALUES (6, 'from B #2')")
        .execute(&b)
        .await
        .unwrap();
    run_sync(&b, &b_tcp).await;

    drain_check(&c, &c_tcp, "C").await;
    assert_eq!(count_notes(&c).await, 6, "C caught up on both of B's blobs");
    assert_eq!(note_body(&c, 5).await, "from B #1");
    assert_eq!(note_body(&c, 6).await, "from B #2");
    println!("[conv] multi-blob spoke-to-spoke OK (C drained 2 of B's blobs)");
    drain_check(&a, &a_tcp, "A").await;

    // 4. Three-way concurrent update on one row converges to a single value.
    sqlx::query("UPDATE notes SET body = 'A wins' WHERE id = 1")
        .execute(&a)
        .await
        .unwrap();
    sqlx::query("UPDATE notes SET body = 'B wins' WHERE id = 1")
        .execute(&b)
        .await
        .unwrap();
    sqlx::query("UPDATE notes SET body = 'C wins' WHERE id = 1")
        .execute(&c)
        .await
        .unwrap();
    println!("[all] updated row 1 concurrently on all three");

    let mut settled = false;
    for _ in 0..MAX_DRAIN {
        sync_and_drain(&a, &a_tcp, "A").await;
        sync_and_drain(&b, &b_tcp, "B").await;
        sync_and_drain(&c, &c_tcp, "C").await;
        let (ba, bb, bc) = (
            note_body(&a, 1).await,
            note_body(&b, 1).await,
            note_body(&c, 1).await,
        );
        if ba == bb && bb == bc {
            // Agreement is not yet convergence: the three sites might match on
            // an intermediate value while the hub still holds a blob that would
            // move one of them. Run one more full round and require the value
            // to be unchanged, so the proof asserts "agreed AND stable" rather
            // than catching a moment that happens to line up.
            sync_and_drain(&a, &a_tcp, "A").await;
            sync_and_drain(&b, &b_tcp, "B").await;
            sync_and_drain(&c, &c_tcp, "C").await;
            let (fa, fb, fc) = (
                note_body(&a, 1).await,
                note_body(&b, 1).await,
                note_body(&c, 1).await,
            );
            assert_eq!(fa, fb, "row 1 diverged A vs B after a settling round");
            assert_eq!(fb, fc, "row 1 diverged B vs C after a settling round");
            assert_eq!(
                fa, ba,
                "row 1 was not stable — agreed on {ba:?} then moved to {fa:?}"
            );
            println!(
                "[conv] three-way concurrent update converged and held (row 1 = {ba:?} on all three)"
            );
            settled = true;
            break;
        }
    }
    assert!(settled, "three-way concurrent update did not converge");

    // All three agree on the whole table, not just the contended row.
    for id in 1..=6 {
        let (ba, bb, bc) = (
            note_body(&a, id).await,
            note_body(&b, id).await,
            note_body(&c, id).await,
        );
        assert_eq!(ba, bb, "row {id} differs A vs B");
        assert_eq!(bb, bc, "row {id} differs B vs C");
    }
    assert_eq!(count_notes(&a).await, 6);
    assert_eq!(count_notes(&b).await, 6);
    assert_eq!(count_notes(&c).await, 6);

    println!("\n=== SYNC-4 GO: three-node convergence over an elected hub ===");

    a.close().await;
    b.close().await;
    c.close().await;
    agent_a.stop().await;
    agent_b.stop().await;
    agent_c.stop().await;
}

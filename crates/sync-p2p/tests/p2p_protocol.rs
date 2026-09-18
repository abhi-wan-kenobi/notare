//! Integration tests for the iroh P2P transport running protocol v2 —
//! allowlist enforcement, whole-frame encryption, session flow, and the
//! driver — all against `FakeSource`s, so `cargo test -p sync-p2p` needs no
//! SQLite at all (the plan's Phase 2 requirement).
//!
//! Driven over real iroh/QUIC (loopback, relay disabled) between two
//! [`P2pAgent`]s with cross-allowlisted identities:
//!   - a full client pull session converges B's changes onto A (and A's onto
//!     B when B runs its own client session — the symmetric pull-only mesh);
//!   - a non-allowlisted node id is refused on the **dial** side (outbound
//!     SSRF gate, §12);
//!   - a non-allowlisted node id is refused on the **accept** side (inbound
//!     SSRF gate, §12);
//!   - a peer that goes offline fails a session **bounded and cleanly**, and
//!     a returning peer (same identity, same node id) is synced again —
//!     no poisoning of later attempts;
//!   - a schema-version mismatch is refused with an Error, never served;
//!   - the driver aggregates per-peer outcomes and only fails a round when
//!     every peer failed.

use std::time::Duration;

use sync_p2p::agent::SYNC_ALPN;
use sync_p2p::source::{Change, ChangeSourceError, Cursor, FakeSource, SqlValue};
use sync_p2p::{Identity, P2pAgent, PeerStore};

/// A tempdir-rooted identity + agent over a shared `FakeSource`, never
/// touching the real `<data_dir>/notare/sync/`.
async fn agent_with_source() -> (tempfile::TempDir, P2pAgent, std::sync::Arc<FakeSource>) {
    let dir = tempfile::tempdir().unwrap();
    let identity = Identity::load_or_create_in(dir.path()).unwrap();
    let peers = PeerStore::load_or_create_in(dir.path()).unwrap();
    let source = FakeSource::shared();
    let agent = P2pAgent::start_with(identity, peers, source.clone())
        .await
        .unwrap();
    (dir, agent, source)
}

/// Wire two agents with cross-allowlisted identities and registered direct
/// addresses, so each can dial the other deterministically offline.
async fn two_allowlisted_agents() -> (
    (tempfile::TempDir, P2pAgent, std::sync::Arc<FakeSource>),
    (tempfile::TempDir, P2pAgent, std::sync::Arc<FakeSource>),
) {
    let (dir_a, agent_a, source_a) = agent_with_source().await;
    let (dir_b, agent_b, source_b) = agent_with_source().await;
    agent_a.peers().add_peer(agent_b.node_id(), "B").unwrap();
    agent_b.peers().add_peer(agent_a.node_id(), "A").unwrap();
    sync_p2p::register_direct_addr(agent_a.node_id(), agent_a.direct_addresses()).await;
    sync_p2p::register_direct_addr(agent_b.node_id(), agent_b.direct_addresses()).await;
    tokio::time::sleep(Duration::from_millis(100)).await;
    ((dir_a, agent_a, source_a), (dir_b, agent_b, source_b))
}

/// One text change, at a given db_version, from the given origin site.
fn text_change(db_version: i64, site: &[u8], val: &str) -> Change {
    Change {
        table: "sessions".into(),
        pk: SqlValue::Text(format!("pk-{db_version}")),
        cid: "title".into(),
        val: SqlValue::Text(val.into()),
        col_version: 1,
        db_version,
        site_id: site.to_vec(),
        cl: 1,
        seq: 1,
    }
}

/// A full symmetric round: A pulls from B, B pulls from A. Both sources
/// converge on the union of the two change logs.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn pull_session_converges_both_directions() {
    let ((_dir_a, agent_a, source_a), (_dir_b, agent_b, source_b)) = two_allowlisted_agents().await;

    // A's identity: its site id, schema, db clock.
    let site_a = b"site-a".to_vec();
    let site_b = b"site-b".to_vec();
    source_a.set_site_id(&site_a).await;
    source_b.set_site_id(&site_b).await;
    source_a.set_schema_version(14).await;
    source_b.set_schema_version(14).await;

    // Only B has changes.
    source_b
        .add_change(text_change(1, &site_b, "hello from B"))
        .await;
    source_b.add_change(text_change(2, &site_b, "second")).await;

    // A pulls from B.
    let outcome = agent_a
        .sync_with_peer(&agent_b.node_id())
        .await
        .expect("A's client session against B");
    assert_eq!(outcome.applied, 2, "A applied both of B's changes");
    assert_eq!(
        outcome.cursor,
        Cursor {
            db_version: 2,
            seq: 1
        },
        "A's cursor for B advanced to B's last change"
    );
    assert_eq!(
        source_a.pull_cursor_for(&site_b).await,
        Some(Cursor {
            db_version: 2,
            seq: 1
        }),
        "the cursor persisted with the applied rows"
    );

    // B pulls from A: A's log now contains B's own rows (origin site B),
    // which the exclude filter drops — nothing to apply, cursor still
    // advances to the end of A's log.
    let outcome_b = agent_b
        .sync_with_peer(&agent_a.node_id())
        .await
        .expect("B's client session against A");
    assert_eq!(outcome_b.applied, 0, "B never re-applies its own rows");
    assert_eq!(
        outcome_b.cursor,
        Cursor {
            db_version: 2,
            seq: 1
        },
        "B's cursor tracks A's log tail (no re-delivery)"
    );

    // Symmetry: A pulls again — nothing new on B.
    let outcome_a2 = agent_a
        .sync_with_peer(&agent_b.node_id())
        .await
        .expect("second pull");
    assert_eq!(outcome_a2.applied, 0, "idempotent re-pull applies nothing");

    agent_a.stop().await;
    agent_b.stop().await;
}

/// The ALPN is the wire contract; protocol v2 bumped it, and the bump is
/// load-bearing (v1 and v2 frames are incompatible).
#[test]
fn alpn_is_v2() {
    assert_eq!(SYNC_ALPN, b"/notare/sync/2");
}

/// Outbound SSRF gate (§12): an unpaired node id is refused before any dial.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn non_allowlisted_peer_refused_on_dial() {
    let (_dir_a, agent_a, _source_a) = agent_with_source().await;
    let (_dir_b, agent_b, _source_b) = agent_with_source().await;

    // A's allowlist is EMPTY — B is not on it. (B allowlists A so only the
    // dial gate is under test.)
    agent_b.peers().add_peer(agent_a.node_id(), "A").unwrap();
    sync_p2p::register_direct_addr(agent_b.node_id(), agent_b.direct_addresses()).await;
    tokio::time::sleep(Duration::from_millis(100)).await;

    let err = agent_a
        .sync_with_peer(&agent_b.node_id())
        .await
        .expect_err("unpaired peer must be refused on dial");
    assert!(
        matches!(err, sync_p2p::AgentError::PeerNotAllowed(_)),
        "refused by the allowlist before dialing, got {err}"
    );

    agent_a.stop().await;
    agent_b.stop().await;
}

/// Inbound SSRF gate (§12): a peer not on OUR allowlist dials us — the
/// handshake completes, but the connection is closed with no streams served.
/// Driven with raw bi-streams (as in v1) because a protocol-level refusal
/// happens below the session.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn non_allowlisted_peer_refused_on_accept() {
    let (_dir_a, agent_a, _source_a) = agent_with_source().await;
    let (_dir_b, agent_b, _source_b) = agent_with_source().await;

    // A allowlists B; B does NOT allowlist A. A dials B; B's accept gate
    // refuses A.
    agent_a.peers().add_peer(agent_b.node_id(), "B").unwrap();
    sync_p2p::register_direct_addr(agent_a.node_id(), agent_a.direct_addresses()).await;
    sync_p2p::register_direct_addr(agent_b.node_id(), agent_b.direct_addresses()).await;
    tokio::time::sleep(Duration::from_millis(100)).await;

    // A's dial passes its own outbound gate (B is allowlisted on A), and
    // B's inbound gate then tears the connection down — A's session fails
    // bounded and cleanly.
    let result = tokio::time::timeout(
        Duration::from_secs(15),
        agent_a.sync_with_peer(&agent_b.node_id()),
    )
    .await
    .expect("refusal must be bounded, not a hang")
    .expect_err("B must refuse the session");
    assert!(
        !matches!(result, sync_p2p::AgentError::PeerNotAllowed(_)),
        "the refusal comes from B's accept gate (inbound), not A's dial gate"
    );

    agent_a.stop().await;
    agent_b.stop().await;
}

/// §15.2 offline-reconnect gate: a peer that goes offline mid-session fails a
/// dial **bounded and cleanly**, and the failure does not poison later
/// attempts — when the peer returns (same identity, same node id), the next
/// session converges.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn offline_peer_fails_bounded_then_reconnects() {
    let ((_dir_a, agent_a, source_a), (dir_b, agent_b, source_b)) = two_allowlisted_agents().await;

    let site_a = b"site-a".to_vec();
    let site_b = b"site-b".to_vec();
    source_a.set_site_id(&site_a).await;
    source_b.set_site_id(&site_b).await;
    source_a.set_schema_version(1).await;
    source_b.set_schema_version(1).await;
    source_b
        .add_change(text_change(1, &site_b, "before offline"))
        .await;

    // 1. B up: A pulls its change.
    let outcome = agent_a.sync_with_peer(&agent_b.node_id()).await.unwrap();
    let node_b = agent_b.node_id();
    assert_eq!(outcome.applied, 1, "B initially reachable");

    // 2. B goes offline — really: its endpoint closes. The stale direct
    //    address stays registered, exactly what a real disconnect looks like.
    agent_b.stop().await;

    // 3. The next session must fail bounded — no hang, no panic. The outer
    //    timeout is the safety valve against a regression that reintroduces
    //    an unbounded retry.
    let failed = tokio::time::timeout(Duration::from_secs(15), agent_a.sync_with_peer(&node_b))
        .await
        .expect("dial to an offline peer must not hang")
        .expect_err("offline peer must fail the session");
    assert!(
        !matches!(failed, sync_p2p::AgentError::PeerNotAllowed(_)),
        "the failure is a dial failure, not an allowlist refusal"
    );

    // 4. B returns: a fresh agent on the SAME identity (same node id — the
    //    device coming back, not a re-pairing).
    let identity_b = Identity::load_or_create_in(dir_b.path()).unwrap();
    let peers_b = PeerStore::load_or_create_in(dir_b.path()).unwrap();
    let agent_b2 = P2pAgent::start_with(identity_b, peers_b, source_b.clone())
        .await
        .unwrap();
    sync_p2p::register_direct_addr(agent_b2.node_id(), agent_b2.direct_addresses()).await;
    tokio::time::sleep(Duration::from_millis(100)).await;

    // 5. The next session converges — and B has a new change since.
    source_b
        .add_change(text_change(2, &site_b, "after return"))
        .await;
    let outcome = agent_a.sync_with_peer(&agent_b2.node_id()).await.unwrap();
    assert_eq!(
        outcome.applied, 1,
        "only the new change — cursor survived the offline gap"
    );
    assert_eq!(
        outcome.cursor,
        Cursor {
            db_version: 2,
            seq: 1
        }
    );

    agent_a.stop().await;
    agent_b2.stop().await;
}

/// A schema-version mismatch is answered with an Error and the session ends
/// — a peer on an older schema is never handed a `cid` it lacks.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn schema_mismatch_is_refused_with_error() {
    let ((_dir_a, agent_a, source_a), (_dir_b, agent_b, source_b)) = two_allowlisted_agents().await;

    let site_a = b"site-a".to_vec();
    let site_b = b"site-b".to_vec();
    source_a.set_site_id(&site_a).await;
    source_b.set_site_id(&site_b).await;
    source_a.set_schema_version(14).await;
    source_b.set_schema_version(13).await;
    source_b
        .add_change(text_change(1, &site_b, "old schema row"))
        .await;

    let err = agent_a
        .sync_with_peer(&agent_b.node_id())
        .await
        .expect_err("mismatched schema must refuse the session");
    assert!(
        err.to_string().contains("schema version mismatch"),
        "the refusal names the mismatch, got {err}"
    );
    // Nothing was applied — A's pull cursor for B is untouched.
    assert_eq!(
        source_a.pull_cursor_for(&site_b).await,
        None,
        "no cursor recorded for a refused session"
    );

    agent_a.stop().await;
    agent_b.stop().await;
}

/// The driver: one round = one client session per allowlisted peer. A peer
/// that is unreachable is recorded, not fatal; the round fails only when
/// every peer failed. A fatal engine error stops the round immediately.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn driver_round_aggregates_and_tolerates_unreachable_peers() {
    let ((_dir_a, _agent_a, source_a), (_dir_b, agent_b, source_b)) =
        two_allowlisted_agents().await;
    source_a.set_site_id(b"site-a").await;
    source_b.set_site_id(b"site-b").await;
    source_a.set_schema_version(1).await;
    source_b.set_schema_version(1).await;
    source_b
        .add_change(text_change(1, b"site-b", "driver row"))
        .await;
    // A third peer: allowlisted on the driver's agent, but never started —
    // unreachable every round.
    let dir_c = tempfile::tempdir().unwrap();
    let identity_c = Identity::load_or_create_in(dir_c.path()).unwrap();

    // The real driver over A: two peers (B reachable, C offline).
    let dir_a_driver = tempfile::tempdir().unwrap();
    let identity_driver = Identity::load_or_create_in(dir_a_driver.path()).unwrap();
    agent_b
        .peers()
        .add_peer(identity_driver.id(), "driver")
        .unwrap();
    let peers_driver = PeerStore::load_or_create_in(dir_a_driver.path()).unwrap();
    // A fresh driver agent shares A's source but needs its own allowlist:
    // B and the offline C.
    peers_driver.add_peer(agent_b.node_id(), "B").unwrap();
    peers_driver
        .add_peer(identity_c.id(), "C (offline)")
        .unwrap();
    let driver_agent = P2pAgent::start_with(identity_driver, peers_driver, source_a.clone())
        .await
        .unwrap();
    sync_p2p::register_direct_addr(agent_b.node_id(), agent_b.direct_addresses()).await;
    let driver = sync_p2p::P2pSyncDriver::new(driver_agent);

    let outcome = driver
        .round()
        .await
        .expect("round with one live peer succeeds");
    assert_eq!(outcome.peers_attempted, 2);
    assert_eq!(outcome.peers_succeeded, 1, "B synced, C offline");
    assert_eq!(outcome.peers_failed, 1, "C's failure recorded, not fatal");
    assert_eq!(outcome.applied, 1, "B's one change applied");
    assert_eq!(outcome.failures.len(), 1);
    assert_eq!(outcome.failures[0].0, identity_c.id());

    // A round where the ONLY peer fails is an error, not a silent wash.
    let dir_solo = tempfile::tempdir().unwrap();
    let identity_solo = Identity::load_or_create_in(dir_solo.path()).unwrap();
    let peers_solo = PeerStore::load_or_create_in(dir_solo.path()).unwrap();
    peers_solo.add_peer(identity_c.id(), "C (offline)").unwrap();
    let solo_agent = P2pAgent::start_with(identity_solo, peers_solo, source_a.clone())
        .await
        .unwrap();
    let solo_driver = sync_p2p::P2pSyncDriver::new(solo_agent);
    let err = solo_driver.round().await.expect_err("all peers failed");
    assert!(matches!(
        err,
        sync_p2p::SyncDriverError::AllPeersFailed(1, _)
    ));

    // A fatal engine error stops the round immediately.
    source_a
        .fail_with(Some(ChangeSourceError::Fatal("engine broken".into())))
        .await;
    let err = driver
        .round()
        .await
        .expect_err("fatal source error stops the round");
    assert!(
        matches!(err, sync_p2p::SyncDriverError::Fatal(_, ref msg) if msg.contains("engine broken"))
    );

    driver.stop().await;
    solo_driver.stop().await;
    agent_b.stop().await;
}

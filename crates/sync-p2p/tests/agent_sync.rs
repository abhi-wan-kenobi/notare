//! End-to-end `P2pAgent` behavior over real iroh/QUIC on loopback (relay
//! disabled, no external network): sync in both directions, paging, resume,
//! the allowlist at dial and accept, and rejection of forged frames.

mod common;

use std::sync::Arc;
use std::time::Duration;

use common::FakeSource;
use iroh::{Endpoint, EndpointAddr, RelayMode};
use sync_p2p::protocol::{FrameReader, FrameWriter};
use sync_p2p::{
    AgentConfig, AgentError, AgentTransport, Identity, Message, P2pAgent, PROTO_VERSION, PeerStore,
    Role, SYNC_ALPN, SessionError, register_direct_addr,
};

struct Device {
    agent: P2pAgent<FakeSource>,
    source: Arc<FakeSource>,
    _dir: tempfile::TempDir,
}

impl Device {
    async fn new(site: u8) -> Self {
        Self::with_tick(site, None).await
    }

    async fn with_tick(site: u8, tick: Option<Duration>) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let identity = Identity::load_or_create_in(dir.path()).unwrap();
        let peers = PeerStore::load_or_create_in(dir.path()).unwrap();
        let source = Arc::new(FakeSource::new(site));
        let config = AgentConfig {
            transport: AgentTransport::Loopback,
            tick_interval: tick,
            ..AgentConfig::default()
        };
        let agent = P2pAgent::start_with_config(identity, peers, Arc::clone(&source), config)
            .await
            .unwrap();
        register_direct_addr(agent.node_id(), agent.direct_addresses()).await;
        Self {
            agent,
            source,
            _dir: dir,
        }
    }

    fn id(&self) -> iroh::PublicKey {
        self.agent.node_id()
    }

    fn z32(&self) -> String {
        self.id().to_z32()
    }

    /// Allowlist `other` on this device.
    fn trust(&self, other: &Device) {
        self.agent.peers().add_peer(other.id(), "peer").unwrap();
    }
}

async fn paired() -> (Device, Device) {
    let a = Device::new(0xA).await;
    let b = Device::new(0xB).await;
    a.trust(&b);
    b.trust(&a);
    (a, b)
}

#[tokio::test]
async fn one_session_moves_changes_in_both_directions() {
    let (a, b) = paired().await;
    a.source.write_many("notes", "a", 3);
    b.source.write_many("notes", "b", 2);

    let report = a.agent.sync_with(&b.id()).await.unwrap();

    assert_eq!(a.source.pks(), ["a0", "a1", "a2", "b0", "b1"]);
    assert_eq!(b.source.pks(), a.source.pks());
    assert_eq!((report.changes_pulled, report.changes_served), (2, 3));
    assert_eq!(a.source.cursor_for(&b.z32()).db_version, 2);
    assert_eq!(b.source.cursor_for(&a.z32()).db_version, 3);

    // Nothing new: a second session transfers nothing and cursors hold.
    let again = b.agent.sync_with(&a.id()).await.unwrap();
    assert_eq!((again.changes_pulled, again.changes_served), (0, 0));
    assert_eq!(a.source.row_count(), 5, "no echo duplicates");
}

#[tokio::test]
async fn multi_page_transfer_over_iroh() {
    let (a, b) = paired().await;
    a.source.write_many("t", "k", 1300);

    let report = b.agent.sync_with(&a.id()).await.unwrap();

    assert_eq!(report.pages_pulled, 3);
    assert_eq!(b.source.row_count(), 1300);
    assert_eq!(b.source.pks(), a.source.pks());
}

#[tokio::test]
async fn interrupted_transfer_resumes_from_the_persisted_cursor() {
    let (a, b) = paired().await;
    a.source.write_many("t", "k", 1300);
    // The serving side dies on its third page.
    a.source.fail_changes_since_on_call(Some(2));

    let err = b.agent.sync_with(&a.id()).await.unwrap_err();
    assert!(
        matches!(err, AgentError::Session(SessionError::Remote { .. })),
        "{err:?}"
    );
    assert_eq!(b.source.row_count(), 1000);
    let saved = b.source.cursor_for(&a.z32());
    assert_eq!(saved.db_version, 1000);

    a.source.fail_changes_since_on_call(None);
    let calls_before = a.source.pull_log().len();
    let report = b.agent.sync_with(&a.id()).await.unwrap();

    assert_eq!(a.source.pull_log()[calls_before].0, saved);
    assert_eq!(report.changes_pulled, 300);
    assert_eq!(b.source.pks(), a.source.pks());
}

#[tokio::test]
async fn dial_to_a_peer_not_on_our_allowlist_is_refused_before_connecting() {
    let a = Device::new(1).await;
    let stranger = Device::new(2).await;
    stranger.trust(&a); // they would accept us; we still must not dial them
    a.source.write("t", "k", "v");

    let err = a.agent.sync_with(&stranger.id()).await.unwrap_err();

    assert!(matches!(err, AgentError::NotAllowlisted), "{err:?}");
    assert_eq!(stranger.source.apply_calls(), 0);
    assert_eq!(stranger.source.changes_since_calls(), 0);
}

#[tokio::test]
async fn inbound_connection_from_a_non_allowlisted_peer_is_refused() {
    let a = Device::new(1).await;
    let intruder = Device::new(2).await;
    intruder.trust(&a); // intruder believes it is paired; `a` never agreed
    intruder.source.write("t", "evil", "v");

    let result = intruder.agent.sync_with(&a.id()).await;

    assert!(result.is_err(), "session must not complete");
    assert_eq!(
        a.source.apply_calls(),
        0,
        "nothing from an unpaired peer is applied"
    );
    assert_eq!(
        a.source.changes_since_calls(),
        0,
        "nothing is served to it either"
    );
    assert_eq!(a.source.row_count(), 0);
}

#[tokio::test]
async fn revoking_a_peer_stops_the_next_session() {
    let (a, b) = paired().await;
    a.source.write("t", "before", "v");
    b.agent.sync_with(&a.id()).await.unwrap();
    assert_eq!(b.source.row_count(), 1);

    a.agent.peers().remove_peer(&b.id()).unwrap();
    a.source.write("t", "after", "v");

    assert!(b.agent.sync_with(&a.id()).await.is_err());
    assert_eq!(b.source.row_count(), 1, "revoked peer receives nothing new");
}

#[tokio::test]
async fn forged_frame_from_an_allowlisted_endpoint_applies_nothing() {
    let a = Device::new(1).await;
    // A raw QUIC endpoint with its own identity, allowlisted by `a`, speaking
    // the protocol by hand.
    let raw_identity = {
        let dir = tempfile::tempdir().unwrap();
        Identity::load_or_create_in(dir.path()).unwrap()
    };
    a.agent.peers().add_peer(raw_identity.id(), "raw").unwrap();
    let endpoint = Endpoint::builder(iroh::endpoint::presets::Minimal)
        .secret_key(raw_identity.secret_key().clone())
        .alpns(vec![SYNC_ALPN.to_vec()])
        .relay_mode(RelayMode::Disabled)
        .bind_addr("127.0.0.1:0")
        .unwrap()
        .bind()
        .await
        .unwrap();
    let mut addr = EndpointAddr::new(a.id());
    for s in a.agent.direct_addresses() {
        addr = addr.with_ip_addr(s);
    }
    let conn = endpoint.connect(addr, SYNC_ALPN).await.unwrap();
    let (mut send, mut recv) = conn.open_bi().await.unwrap();

    // A valid Hello, so the agent's session is past the handshake...
    let mut hello_writer = FrameWriter::new(
        &mut send,
        raw_identity.secret_key().clone(),
        a.id(),
        Role::Dialer,
    );
    hello_writer
        .send(&Message::Hello {
            device_id: raw_identity.id().to_z32(),
            site_id: vec![0xEE; 16],
            proto: PROTO_VERSION,
        })
        .await
        .unwrap();
    // ...then a page sealed with a key that is not ours.
    let forger = iroh::SecretKey::generate();
    let mut forged = FrameWriter::new(&mut send, forger, a.id(), Role::Dialer);
    forged
        .send(&Message::Changes(sync_p2p::WirePage {
            changes: vec![],
            next: sync_p2p::WireCursor {
                db_version: 1,
                seq: 0,
            },
            more: false,
        }))
        .await
        .unwrap();

    // The agent drops the session: its side of the stream ends.
    let mut reader = FrameReader::new(
        &mut recv,
        raw_identity.secret_key().clone(),
        a.id(),
        Role::Dialer,
    );
    let outcome = tokio::time::timeout(Duration::from_secs(5), async {
        while let Ok(Some(_)) = reader.recv().await {}
    })
    .await;
    assert!(
        outcome.is_ok(),
        "agent must end the session after a forged frame"
    );
    assert_eq!(a.source.apply_calls(), 0);
    assert_eq!(
        a.source.cursor_for(&raw_identity.id().to_z32()),
        sync_p2p::WireCursor::default()
    );
    conn.close(0u32.into(), b"done");
    endpoint.close().await;
}

#[tokio::test]
async fn periodic_tick_syncs_allowlisted_peers_without_a_manual_call() {
    let a = Device::with_tick(1, Some(Duration::from_millis(100))).await;
    let b = Device::new(2).await;
    a.trust(&b);
    b.trust(&a);
    a.source.write("t", "from-a", "v");
    b.source.write("t", "from-b", "v");

    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    while (a.source.row_count() < 2 || b.source.row_count() < 2)
        && tokio::time::Instant::now() < deadline
    {
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert_eq!(a.source.pks(), ["from-a", "from-b"]);
    assert_eq!(b.source.pks(), ["from-a", "from-b"]);
}

#[tokio::test]
async fn a_peer_that_never_answers_does_not_hang_the_caller_forever() {
    // `ghost` is allowlisted but its address is a dead port: the dial ladder
    // must give up on its own schedule.
    let a = Device::new(1).await;
    let ghost = Identity::load_or_create_in(tempfile::tempdir().unwrap().path()).unwrap();
    a.agent.peers().add_peer(ghost.id(), "ghost").unwrap();
    register_direct_addr(ghost.id(), vec!["127.0.0.1:9".parse().unwrap()]).await;

    let err = tokio::time::timeout(Duration::from_secs(30), a.agent.sync_with(&ghost.id()))
        .await
        .expect("dial ladder is bounded")
        .unwrap_err();
    assert!(matches!(err, AgentError::Dial(_)), "{err:?}");
}

#[tokio::test]
async fn pairing_requires_an_active_offer_and_the_offer_secret() {
    let offerer = Device::new(1).await;
    let joiner = Device::new(2).await;
    let offer = offerer
        .agent
        .pairing()
        .create_offer(&offerer.id(), "offerer");

    // A wrong secret is refused and nothing becomes pending.
    let mut forged = offer.clone();
    forged.secret[0] ^= 0xFF;
    let err = joiner.agent.pair_with(&forged).await.unwrap_err();
    assert!(matches!(err, AgentError::Pairing(_)), "{err:?}");
    assert!(
        offerer
            .agent
            .pairing()
            .status()
            .pending_confirmations
            .is_empty()
    );

    // The real secret works; both sides derive the same SAS; the joiner is
    // pending confirmation, NOT yet allowlisted.
    let outcome = joiner.agent.pair_with(&offer).await.unwrap();
    assert_eq!(outcome.offerer.device_id, offerer.z32());
    let pending = offerer.agent.pairing().status().pending_confirmations;
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].device_id, joiner.z32());
    assert_eq!(pending[0].sas, outcome.sas);
    assert!(!offerer.agent.peers().is_allowed(&joiner.id()));

    // The offer was single-use: the connection is refused now.
    let err = joiner.agent.pair_with(&offer).await.unwrap_err();
    assert!(matches!(err, AgentError::Pairing(_)), "{err:?}");
}

#[tokio::test]
async fn pairing_alpn_does_not_open_the_sync_protocol_to_strangers() {
    let offerer = Device::new(1).await;
    let stranger = Device::new(2).await;
    offerer.source.write("t", "private", "v");
    stranger.trust(&offerer);
    let _offer = offerer.agent.pairing().create_offer(&offerer.id(), "o");

    // An active offer must not let an unpaired peer reach sync.
    assert!(stranger.agent.sync_with(&offerer.id()).await.is_err());
    assert_eq!(offerer.source.changes_since_calls(), 0);
    assert_eq!(stranger.source.row_count(), 0);
}

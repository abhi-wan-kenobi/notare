//! `SyncSession` behavior over in-memory streams: both directions, paging,
//! cursors, resume, and refusal of misbehaving peers.

mod common;

use std::sync::Arc;
use std::time::Duration;

use common::{FakeSource, change_cursor};
use iroh::SecretKey;
use sync_p2p::protocol::{FrameReader, FrameWriter};
use sync_p2p::{
    ErrorCode, Message, PROTO_VERSION, ProtocolError, Role, SessionConfig, SessionError,
    SessionReport, SyncSession, WireChange, WireCursor, WirePage, WireValue,
};
use tokio::io::duplex;
use tokio::sync::Mutex;

struct Node {
    secret: SecretKey,
    source: Arc<FakeSource>,
}

impl Node {
    fn new(site: u8) -> Self {
        Self {
            secret: SecretKey::generate(),
            source: Arc::new(FakeSource::new(site)),
        }
    }

    fn id(&self) -> String {
        self.secret.public().to_z32()
    }
}

fn cfg(page: usize) -> SessionConfig {
    SessionConfig {
        page_size: page,
        max_page_size: 5000,
        message_timeout: Duration::from_secs(5),
    }
}

type Outcome = Result<SessionReport, SessionError>;
type Locks = (Arc<Mutex<()>>, Arc<Mutex<()>>);

/// Run `a` (dialer) against `b` (acceptor) to completion.
async fn sync_pair(a: &Node, b: &Node, page: usize) -> (Outcome, Outcome) {
    sync_pair_locked(a, b, page, None).await
}

async fn sync_pair_locked(
    a: &Node,
    b: &Node,
    page: usize,
    locks: Option<Locks>,
) -> (Outcome, Outcome) {
    let (a_send, b_recv) = duplex(1 << 16);
    let (b_send, a_recv) = duplex(1 << 16);
    let mut sa = SyncSession::new(
        Arc::clone(&a.source),
        a.secret.clone(),
        b.secret.public(),
        Role::Dialer,
        cfg(page),
    );
    let mut sb = SyncSession::new(
        Arc::clone(&b.source),
        b.secret.clone(),
        a.secret.public(),
        Role::Acceptor,
        cfg(page),
    );
    if let Some((la, lb)) = locks {
        sa = sa.with_apply_lock(la);
        sb = sb.with_apply_lock(lb);
    }
    tokio::join!(sa.run(a_recv, a_send), sb.run(b_recv, b_send))
}

#[tokio::test]
async fn changes_flow_both_ways_and_cursors_advance() {
    let a = Node::new(0xA);
    let b = Node::new(0xB);
    a.source.write_many("notes", "a", 3);
    b.source.write_many("notes", "b", 2);

    let (ra, rb) = sync_pair(&a, &b, 500).await;
    let (ra, rb) = (ra.unwrap(), rb.unwrap());

    assert_eq!(a.source.pks(), ["a0", "a1", "a2", "b0", "b1"]);
    assert_eq!(b.source.pks(), a.source.pks());
    assert_eq!((ra.changes_pulled, ra.changes_served), (2, 3));
    assert_eq!((rb.changes_pulled, rb.changes_served), (3, 2));
    assert_eq!(ra.peer_site_id, b.source.site());

    // Each cursor is the last row *the peer* served: B's b1 was its 2nd
    // version, A's a2 its 3rd.
    assert_eq!(
        a.source.cursor_for(&b.id()),
        WireCursor {
            db_version: 2,
            seq: 0
        }
    );
    assert_eq!(
        b.source.cursor_for(&a.id()),
        WireCursor {
            db_version: 3,
            seq: 0
        }
    );
    // Each side excluded the other's site when serving: no echo.
    assert!(
        a.source
            .pull_log()
            .iter()
            .all(|(_, ex, _)| ex.as_deref() == Some(&b.source.site()[..]))
    );
}

#[tokio::test]
async fn multi_page_transfer_delivers_every_row_once_in_order() {
    let a = Node::new(1);
    let b = Node::new(2);
    a.source.write_many("t", "k", 1200);

    let (ra, rb) = sync_pair(&a, &b, 500).await;
    let (ra, rb) = (ra.unwrap(), rb.unwrap());

    assert_eq!(rb.pages_pulled, 3, "500 + 500 + 200");
    assert_eq!(rb.changes_pulled, 1200);
    assert_eq!(ra.pages_served, 3);
    assert_eq!(b.source.row_count(), 1200);
    assert_eq!(b.source.apply_calls(), 3);
    // Pull cursors chain: each Pull starts where the previous page ended.
    let log = a.source.pull_log();
    assert_eq!(log[0].0, WireCursor::default());
    assert_eq!(log[1].0.db_version, 500);
    assert_eq!(log[2].0.db_version, 1000);
    assert!(log.iter().all(|(_, _, limit)| *limit == 500));
}

#[tokio::test]
async fn second_session_pulls_only_what_is_new() {
    let a = Node::new(1);
    let b = Node::new(2);
    a.source.write_many("t", "k", 10);
    sync_pair(&a, &b, 500).await.1.unwrap();
    let after_first = b.source.cursor_for(&a.id());

    a.source.write("t", "late", "v");
    let (_, rb) = sync_pair(&a, &b, 500).await;
    let rb = rb.unwrap();

    assert_eq!(rb.changes_pulled, 1);
    assert_eq!(a.source.pull_log().last().unwrap().0, after_first);
    assert_eq!(b.source.row_count(), 11);
}

#[tokio::test]
async fn interrupted_transfer_resumes_from_the_persisted_cursor() {
    let a = Node::new(1);
    let b = Node::new(2);
    a.source.write_many("t", "k", 1200);

    // B's third `apply` (rows 1001-1200 of the stream... 3rd page) fails.
    b.source.fail_apply_on_page(Some(2));
    let (ra, rb) = sync_pair(&a, &b, 500).await;
    assert!(matches!(rb, Err(SessionError::Source(_))), "{rb:?}");
    assert!(ra.is_err(), "the serving side must not report success");

    assert_eq!(b.source.row_count(), 1000);
    let saved = b.source.cursor_for(&a.id());
    assert_eq!(
        saved.db_version, 1000,
        "cursor covers exactly the applied pages"
    );

    b.source.fail_apply_on_page(None);
    let served_before = a.source.pull_log().len();
    let (_, rb) = sync_pair(&a, &b, 500).await;
    let rb = rb.unwrap();

    assert_eq!(
        a.source.pull_log()[served_before].0,
        saved,
        "resume must start at the persisted cursor, not at zero"
    );
    assert_eq!(rb.changes_pulled, 200, "only the remainder is re-sent");
    assert_eq!(b.source.row_count(), 1200);
    assert_eq!(b.source.pks(), a.source.pks());
}

#[tokio::test]
async fn serving_failure_reaches_the_peer_without_leaking_internals() {
    let a = Node::new(1);
    let b = Node::new(2);
    a.source.write_many("t", "k", 5);
    a.source.fail_changes_since_on_call(Some(0));

    let (ra, rb) = sync_pair(&a, &b, 500).await;
    assert!(matches!(ra, Err(SessionError::Source(_))));
    match rb {
        Err(SessionError::Remote { code, msg }) => {
            assert_eq!(code, ErrorCode::Internal);
            assert!(!msg.contains("secret") && !msg.contains("SELECT"), "{msg}");
        }
        other => panic!("expected Remote error, got {other:?}"),
    }
    assert_eq!(b.source.row_count(), 0);
}

#[tokio::test]
async fn concurrent_sessions_never_move_the_stored_cursor_backwards() {
    let a = Node::new(1);
    let b = Node::new(2);
    a.source.write_many("t", "k", 1500);
    let lock_a = Arc::new(Mutex::new(()));
    let lock_b = Arc::new(Mutex::new(()));

    let run = || sync_pair_locked(&a, &b, 100, Some((lock_a.clone(), lock_b.clone())));
    let (r1, r2) = tokio::join!(run(), run());
    r1.0.unwrap();
    r1.1.unwrap();
    r2.0.unwrap();
    r2.1.unwrap();

    let hist = b.source.cursor_history();
    assert!(
        hist.windows(2).all(|w| w[0] < w[1]),
        "cursor writes must be strictly increasing: {hist:?}"
    );
    assert_eq!(
        b.source.row_count(),
        1500,
        "no duplicates despite two sessions"
    );
}

// ---------------------------------------------------------------------------
// misbehaving peers
// ---------------------------------------------------------------------------

/// A hand-driven peer on the other end of a real `SyncSession` (acceptor
/// role); lets a test send arbitrary, even invalid, protocol messages.
struct RawPeer {
    secret: SecretKey,
    session_pub: iroh::PublicKey,
    w: FrameWriter<tokio::io::DuplexStream>,
    r: FrameReader<tokio::io::DuplexStream>,
}

impl RawPeer {
    /// Starts a dialer session against `source`; returns the raw acceptor end
    /// and the session's result future.
    fn start(
        source: Arc<FakeSource>,
        config: SessionConfig,
    ) -> (Self, tokio::task::JoinHandle<Outcome>) {
        let session_secret = SecretKey::generate();
        let raw_secret = SecretKey::generate();
        let (s_send, raw_recv) = duplex(1 << 16);
        let (raw_send, s_recv) = duplex(1 << 16);
        let session = SyncSession::new(
            source,
            session_secret.clone(),
            raw_secret.public(),
            Role::Dialer,
            config,
        );
        let handle = tokio::spawn(session.run(s_recv, s_send));
        let r = FrameReader::new(
            raw_recv,
            raw_secret.clone(),
            session_secret.public(),
            Role::Acceptor,
        );
        (
            Self {
                secret: raw_secret.clone(),
                session_pub: session_secret.public(),
                w: FrameWriter::new(
                    raw_send,
                    raw_secret.clone(),
                    session_secret.public(),
                    Role::Acceptor,
                ),
                r,
            },
            handle,
        )
    }

    fn writer(&mut self) -> &mut FrameWriter<tokio::io::DuplexStream> {
        &mut self.w
    }

    fn hello(&self) -> Message {
        Message::Hello {
            device_id: self.secret.public().to_z32(),
            site_id: vec![0xEE; 16],
            proto: PROTO_VERSION,
        }
    }

    /// Complete the Hello exchange and consume the session's first Pull.
    async fn handshake(&mut self) -> (WireCursor, u32) {
        assert!(matches!(
            self.r.recv().await.unwrap(),
            Some(Message::Hello { .. })
        ));
        let hello = self.hello();
        self.writer().send(&hello).await.unwrap();
        match self.r.recv().await.unwrap() {
            Some(Message::Pull { after, limit }) => (after, limit),
            other => panic!("expected Pull, got {other:?}"),
        }
    }
}

fn row(i: i64) -> WireChange {
    WireChange {
        table: "t".into(),
        pk: WireValue::Text(format!("p{i}")),
        cid: "v".into(),
        val: WireValue::Null,
        col_version: 1,
        db_version: i,
        site_id: vec![0xEE; 16],
        cl: 1,
        seq: 0,
    }
}

fn page(n: i64, more: bool) -> Message {
    let changes: Vec<WireChange> = (1..=n).map(row).collect();
    let next = changes.last().map(change_cursor).unwrap();
    Message::Changes(WirePage {
        changes,
        next,
        more,
    })
}

#[tokio::test]
async fn frame_not_authenticated_by_the_peer_key_is_rejected_and_applies_nothing() {
    let source = Arc::new(FakeSource::new(1));
    let (mut peer, handle) = RawPeer::start(Arc::clone(&source), cfg(10));
    let (_, _) = peer.handshake().await;

    // Sealed with a key that is not the peer's: stands in for flipped
    // ciphertext bits (the AEAD tag check is the same failure).
    let impostor = SecretKey::generate();
    let mut forged = FrameWriter::new(peer.w.get_mut(), impostor, peer.session_pub, Role::Acceptor);
    forged.send(&page(3, false)).await.unwrap();

    let result = handle.await.unwrap();
    assert!(
        matches!(
            result,
            Err(SessionError::Protocol(ProtocolError::Crypto(_)))
        ),
        "{result:?}"
    );
    assert_eq!(source.apply_calls(), 0);
    assert_eq!(
        source.cursor_for(&peer.secret.public().to_z32()),
        WireCursor::default()
    );
}

#[tokio::test]
async fn hello_claiming_another_device_id_is_refused() {
    let source = Arc::new(FakeSource::new(1));
    let (mut peer, handle) = RawPeer::start(source, cfg(10));
    assert!(matches!(
        peer.r.recv().await.unwrap(),
        Some(Message::Hello { .. })
    ));
    let lie = Message::Hello {
        device_id: SecretKey::generate().public().to_z32(),
        site_id: vec![9; 16],
        proto: PROTO_VERSION,
    };
    peer.writer().send(&lie).await.unwrap();

    assert!(matches!(
        handle.await.unwrap(),
        Err(SessionError::Unsupported(_))
    ));
    // The session tells the peer why before closing.
    assert!(matches!(
        peer.r.recv().await.unwrap(),
        Some(Message::Error {
            code: ErrorCode::Unsupported,
            ..
        })
    ));
}

#[tokio::test]
async fn wrong_protocol_version_is_refused() {
    let source = Arc::new(FakeSource::new(1));
    let (mut peer, handle) = RawPeer::start(source, cfg(10));
    assert!(matches!(
        peer.r.recv().await.unwrap(),
        Some(Message::Hello { .. })
    ));
    let hello = Message::Hello {
        device_id: peer.secret.public().to_z32(),
        site_id: vec![9; 16],
        proto: 1,
    };
    peer.writer().send(&hello).await.unwrap();
    assert!(matches!(
        handle.await.unwrap(),
        Err(SessionError::Unsupported(_))
    ));
}

#[tokio::test]
async fn peer_with_our_own_site_id_is_refused() {
    let source = Arc::new(FakeSource::new(7));
    let (mut peer, handle) = RawPeer::start(Arc::clone(&source), cfg(10));
    assert!(matches!(
        peer.r.recv().await.unwrap(),
        Some(Message::Hello { .. })
    ));
    let clone = Message::Hello {
        device_id: peer.secret.public().to_z32(),
        site_id: source.site(),
        proto: PROTO_VERSION,
    };
    peer.writer().send(&clone).await.unwrap();
    assert!(matches!(
        handle.await.unwrap(),
        Err(SessionError::Unsupported(_))
    ));
}

#[tokio::test]
async fn unsolicited_changes_are_a_violation() {
    let source = Arc::new(FakeSource::new(1));
    let (mut peer, handle) = RawPeer::start(Arc::clone(&source), cfg(10));
    peer.handshake().await;
    peer.writer().send(&page(2, false)).await.unwrap(); // answers the Pull
    // ...and now a second page nobody asked for.
    peer.writer().send(&page(2, false)).await.unwrap();

    assert!(matches!(
        handle.await.unwrap(),
        Err(SessionError::Violation(_))
    ));
    assert_eq!(source.row_count(), 2, "only the requested page was applied");
}

#[tokio::test]
async fn page_larger_than_requested_is_rejected_unapplied() {
    let source = Arc::new(FakeSource::new(1));
    let (mut peer, handle) = RawPeer::start(Arc::clone(&source), cfg(10));
    let (_, limit) = peer.handshake().await;
    assert_eq!(limit, 10);
    peer.writer().send(&page(11, false)).await.unwrap();

    assert!(matches!(
        handle.await.unwrap(),
        Err(SessionError::Violation(_))
    ));
    assert_eq!(source.apply_calls(), 0);
}

#[tokio::test]
async fn page_that_does_not_advance_the_cursor_cannot_loop_the_puller() {
    let source = Arc::new(FakeSource::new(1));
    let (mut peer, handle) = RawPeer::start(Arc::clone(&source), cfg(10));
    let (after, _) = peer.handshake().await;
    let stuck = Message::Changes(WirePage {
        changes: vec![],
        next: after,
        more: true,
    });
    peer.writer().send(&stuck).await.unwrap();

    assert!(matches!(
        handle.await.unwrap(),
        Err(SessionError::Violation(_))
    ));
}

#[tokio::test]
async fn silent_peer_times_out() {
    let source = Arc::new(FakeSource::new(1));
    let config = SessionConfig {
        message_timeout: Duration::from_millis(150),
        ..cfg(10)
    };
    let (mut peer, handle) = RawPeer::start(source, config);
    assert!(matches!(
        peer.r.recv().await.unwrap(),
        Some(Message::Hello { .. })
    ));
    // Never answer.
    let result = tokio::time::timeout(Duration::from_secs(3), handle)
        .await
        .unwrap()
        .unwrap();
    assert!(
        matches!(result, Err(SessionError::Timeout(_))),
        "{result:?}"
    );
}

#[tokio::test]
async fn peer_closing_mid_session_is_a_clean_error() {
    let source = Arc::new(FakeSource::new(1));
    let (mut peer, handle) = RawPeer::start(Arc::clone(&source), cfg(10));
    peer.handshake().await;
    drop(peer);
    let result = handle.await.unwrap();
    assert!(matches!(result, Err(SessionError::Closed)), "{result:?}");
    assert_eq!(source.apply_calls(), 0);
}

#[tokio::test]
async fn pull_limit_is_clamped_and_oversize_pages_are_halved() {
    // A peer asking for 1_000_000 rows is served at most `max_page_size`.
    let source = Arc::new(FakeSource::new(1));
    source.write_many("t", "k", 30);
    let config = SessionConfig {
        max_page_size: 7,
        ..cfg(10)
    };
    let (mut peer, handle) = RawPeer::start(Arc::clone(&source), config);
    peer.handshake().await;
    peer.writer()
        .send(&Message::Pull {
            after: WireCursor::default(),
            limit: 1_000_000,
        })
        .await
        .unwrap();
    match peer.r.recv().await.unwrap() {
        Some(Message::Changes(p)) => {
            assert_eq!(p.changes.len(), 7);
            assert!(p.more);
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(source.pull_log().last().unwrap().2, 7);

    // limit 0 still makes progress (clamped up to 1).
    peer.writer()
        .send(&Message::Pull {
            after: WireCursor::default(),
            limit: 0,
        })
        .await
        .unwrap();
    match peer.r.recv().await.unwrap() {
        Some(Message::Changes(p)) => assert_eq!(p.changes.len(), 1),
        other => panic!("{other:?}"),
    }
    handle.abort();
}

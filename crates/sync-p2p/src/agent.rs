//! The P2P sync agent — the iroh/QUIC endpoint that runs protocol v2 against
//! allowlisted peers, backed by a [`ChangeSource`].
//!
//! ## What changed in the 0.7 engine swap
//!
//! cr-sqlite deleted the entire C network layer
//! (`crates/cloudsync/build/network_p2p.c`): there is no longer a synchronous
//! C `network_send_buffer`/`network_receive_buffer` boundary that needed a
//! localhost TCP listener, a bearer token, and request relay. The agent now
//! speaks protocol v2 ([`crate::protocol`]) directly, on both sides of the
//! connection: **client** toward every allowlisted peer each tick
//! ([`P2pAgent::sync_with_peer`]), **server** for accepted inbound streams
//! ([`serve_sync_stream`] — the same session, mirrored).
//!
//! Kept unchanged from v1 (plan Phase 2 "keep"): [`crate::identity`],
//! [`crate::peers`], [`crate::crypto`], endpoint construction, the
//! [`dial_peer`] ladder, the per-connection and per-stream allowlist checks
//! on accept, the direct-addr registry, and `stop`.
//!
//! ## Allowlist enforcement
//!
//! Every outbound dial and every inbound connection is checked against the
//! [`PeerStore`]; a node id not on the allowlist is refused in both
//! directions — and a peer revoked mid-connection is re-checked on every
//! bi-stream of a still-open connection.

use std::sync::Arc;

use iroh::endpoint::{RecvStream, SendStream};
use iroh::{Endpoint, EndpointAddr, PublicKey, RelayMode};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::crypto;
use crate::identity::Identity;
use crate::peers::PeerStore;
use crate::protocol::{DEFAULT_MAX_BYTES, MAX_FRAME_BYTES, SyncMessage};
use crate::source::{ChangeSource, ChangeSourceError, Cursor, PeerSyncOutcome};

/// The ALPN the sync transport speaks over iroh. Bumped to `/2` with protocol
/// v2: the wire format is incompatible with v1, and peers must refuse each
/// other at the TLS layer rather than misparse frames.
///
/// Public because it is part of the wire contract: any peer implementation —
/// and the allowlist regression test, which opens raw bi-streams on a single
/// connection — has to negotiate the same ALPN.
pub const SYNC_ALPN: &[u8] = b"/notare/sync/2";

/// Errors from starting or running the P2P agent, or from a sync session.
#[derive(Debug, thiserror::Error)]
pub enum AgentError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Iroh(#[from] iroh::endpoint::BindError),
    #[error("invalid bind address")]
    BadBindAddr,
    #[error(transparent)]
    Identity(#[from] crate::identity::IdentityError),
    #[error(transparent)]
    Peers(#[from] crate::peers::PeersError),
    #[error(transparent)]
    Crypto(#[from] crate::crypto::CryptoError),
    #[error(transparent)]
    Source(#[from] ChangeSourceError),
    #[error("peer {0} is not allowlisted")]
    PeerNotAllowed(PublicKey),
    #[error("sync session with peer {0} failed: {1}")]
    Session(PublicKey, String),
}

/// A running P2P sync agent: the in-process iroh endpoint, the peer
/// allowlist, and the [`ChangeSource`] it pulls and serves through.
pub struct P2pAgent {
    /// The iroh endpoint — owns the QUIC socket and the secret key.
    endpoint: Endpoint,
    /// This device's identity (node id = `endpoint.id()`).
    identity: Identity,
    /// The peer allowlist (enforced at dial + accept).
    peers: PeerStore,
    /// The transport mode this agent dials with (drives the dial ladder).
    transport: AgentTransport,
    /// The DB seam this agent syncs through.
    source: Arc<dyn ChangeSource>,
    /// The join handle for the iroh inbound connection accept loop.
    iroh_handle: tokio::task::JoinHandle<()>,
}

/// How the agent's iroh endpoint finds and is found by peers. See
/// `docs/internal/sync-p2p.md` §20 for the full rationale.
///
/// Deliberately an explicit enum rather than a bool: the two modes differ in
/// more than one axis (relay, bind address, address-lookup publishing), and
/// a bool would invite someone to thread only one of those through later and
/// silently produce a hybrid that is neither offline-safe nor discoverable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentTransport {
    /// Same-machine proof / test behavior: `RelayMode::Disabled`, bound to
    /// `127.0.0.1` only, peer addresses resolved from the process-local
    /// `DIRECT_ADDRS` registry populated by [`register_direct_addr`].
    /// Deterministic and fully offline — what every test in this crate uses.
    Loopback,
    /// Production: `RelayMode::Default`, bound on all interfaces (dual-stack —
    /// the default when no explicit `bind_addr` is set), and iroh's DNS/pkarr
    /// address-lookup service enabled so the endpoint both publishes its own
    /// address record and resolves peers by node id alone. Reaches n0's
    /// infrastructure on the open internet — see §20 before assuming this is
    /// free of privacy characteristics worth documenting.
    Discovered,
}

impl P2pAgent {
    /// Start an agent using the persisted identity + allowlist from the app
    /// data dir (`<data_dir>/notare/sync/`), with real relay/DNS discovery
    /// enabled (`AgentTransport::Discovered`). The app entry point.
    pub async fn start(source: Arc<dyn ChangeSource>) -> Result<Self, AgentError> {
        let identity = Identity::load_or_create()?;
        let peers = PeerStore::load_or_create()?;
        Self::start_with_transport(identity, peers, source, AgentTransport::Discovered).await
    }

    /// Start with an explicit identity + peer store, on
    /// `AgentTransport::Loopback` (same-machine, deterministic, offline).
    /// The path tests use.
    pub async fn start_with(
        identity: Identity,
        peers: PeerStore,
        source: Arc<dyn ChangeSource>,
    ) -> Result<Self, AgentError> {
        Self::start_with_transport(identity, peers, source, AgentTransport::Loopback).await
    }

    /// Start with an explicit identity, peer store, change source, and
    /// transport config.
    pub async fn start_with_transport(
        identity: Identity,
        peers: PeerStore,
        source: Arc<dyn ChangeSource>,
        transport: AgentTransport,
    ) -> Result<Self, AgentError> {
        // The secret key IS the device identity key — iroh's NodeId derives
        // from it — in both modes.
        let endpoint = match transport {
            // Disable relay for the same-machine proof (deterministic, no
            // external network), bind localhost only.
            AgentTransport::Loopback => {
                Endpoint::builder(iroh::endpoint::presets::Minimal)
                    .secret_key(identity.secret_key().clone())
                    .alpns(vec![SYNC_ALPN.to_vec()])
                    .relay_mode(RelayMode::Disabled)
                    .bind_addr("127.0.0.1:0")
                    .map_err(|_| AgentError::BadBindAddr)?
                    .bind()
                    .await?
            }
            // `presets::N0` wires up the n0 DNS/pkarr address-lookup service
            // (publish + resolve) and n0's relay servers (confirmed against
            // the vendored iroh 1.1.0 source, `iroh-1.1.0/src/endpoint/presets.rs`;
            // discovery was renamed `address_lookup` in this version, so
            // `presets::Minimal` — which sets nothing but the TLS crypto
            // provider — does NOT include it). `relay_mode` is set explicitly
            // so this endpoint is `RelayMode::Default` even if
            // `IROH_FORCE_STAGING_RELAYS` is set in the environment. No
            // `bind_addr` call: the builder's unspecified defaults bind
            // dual-stack on every interface.
            AgentTransport::Discovered => {
                Endpoint::builder(iroh::endpoint::presets::N0)
                    .secret_key(identity.secret_key().clone())
                    .alpns(vec![SYNC_ALPN.to_vec()])
                    .relay_mode(RelayMode::Default)
                    .bind()
                    .await?
            }
        };

        // iroh inbound accept loop: remote peers dial us to pull our changes.
        let iroh_endpoint = endpoint.clone();
        let iroh_peers = peers.clone();
        let iroh_identity = identity.clone();
        let iroh_source = Arc::clone(&source);
        let iroh_handle = tokio::spawn(async move {
            accept_iroh(iroh_endpoint, iroh_peers, iroh_source, iroh_identity).await;
        });

        Ok(Self {
            endpoint,
            identity,
            peers,
            transport,
            source,
            iroh_handle,
        })
    }

    /// This device's node id / iroh EndpointId.
    pub fn node_id(&self) -> PublicKey {
        self.identity.id()
    }

    /// The peer allowlist (for adding/removing peers at runtime).
    pub fn peers(&self) -> &PeerStore {
        &self.peers
    }

    /// The iroh endpoint's bound direct addresses (so a peer can dial us with
    /// a direct address rather than via relay — used by tests to build the
    /// dial address without external discovery).
    pub fn direct_addresses(&self) -> Vec<std::net::SocketAddr> {
        self.endpoint.bound_sockets()
    }

    /// Gracefully stop the agent.
    pub async fn stop(self) {
        self.iroh_handle.abort();
        self.endpoint.close().await;
    }

    /// Run one pull-only sync session as **client** against `node_id`: Hello →
    /// Pull{after: cursor[peer]} → apply each page (the source advances the
    /// cursor with the applied rows) → Ack when `more == false`. Enforces the
    /// allowlist before dialing — the outbound half of the SSRF fix.
    ///
    /// An unreachable peer is an ordinary, bounded error (the dial ladder
    /// caps it); the caller (the driver's round) decides how to aggregate.
    pub async fn sync_with_peer(&self, node_id: &PublicKey) -> Result<PeerSyncOutcome, AgentError> {
        if !self.peers.is_allowed(node_id) {
            return Err(AgentError::PeerNotAllowed(*node_id));
        }
        let conn = dial_peer(node_id, &self.endpoint, self.transport).await?;
        let outcome = tokio::time::timeout(
            std::time::Duration::from_secs(120),
            client_session(&conn, self.identity.secret_key(), node_id, &*self.source),
        )
        .await
        .map_err(|_| AgentError::Session(*node_id, "sync session timed out".into()))??;
        self.peers.touch_last_seen(node_id);
        Ok(outcome)
    }
}

// ---------------------------------------------------------------------------
// Session logic (pull-only, symmetric)
// ---------------------------------------------------------------------------

/// One protocol v2 client session on an open connection. Whole frames are
/// encrypted with the per-peer key ([`crate::crypto`]); the bi-stream is
/// finished at the end of the session.
///
/// Returns the aggregate outcome (rows applied, final cursor) so the driver
/// can report per-peer progress.
async fn client_session(
    conn: &iroh::endpoint::Connection,
    secret: &iroh::SecretKey,
    peer_id: &PublicKey,
    source: &dyn ChangeSource,
) -> Result<PeerSyncOutcome, AgentError> {
    let (mut send, mut recv) = conn
        .open_bi()
        .await
        .map_err(|e| AgentError::Session(*peer_id, format!("open stream: {e}")))?;

    // 1. Hello: our site id, db version, schema version. The server answers
    //    with its own Hello (which also carries *its* site id — the cursor
    //    key) or an Error.
    let site_id = source.site_id().await?;
    let schema_version = source.schema_version().await?;
    let db_version = source.db_version().await?;
    let mut stream = EncryptedStream::new(secret, peer_id, &mut send, &mut recv);
    stream
        .send(&SyncMessage::Hello {
            site_id: site_id.clone(),
            db_version,
            schema_version,
        })
        .await?;

    let peer_site = match stream.receive().await? {
        SyncMessage::Hello {
            site_id: peer_site,
            schema_version: peer_schema,
            ..
        } => {
            if peer_site.is_empty() {
                return Err(AgentError::Session(
                    *peer_id,
                    "peer sent an empty site id".into(),
                ));
            }
            if peer_schema != schema_version {
                let message =
                    format!("schema version mismatch: ours {schema_version}, peer {peer_schema}");
                let _ = stream
                    .send(&SyncMessage::Error {
                        message: message.clone(),
                    })
                    .await;
                return Err(AgentError::Session(*peer_id, message));
            }
            peer_site
        }
        SyncMessage::Error { message } => {
            return Err(AgentError::Session(*peer_id, message));
        }
        other => {
            return Err(AgentError::Session(
                *peer_id,
                format!("expected Hello, got {other:?}"),
            ));
        }
    };

    // 2. Pull pages until `more == false`, applying each through the source
    //    (the cursor advance commits with the rows, per the plan's
    //    "cursor must commit with the applied page" invariant).
    let mut applied = 0usize;
    let mut cursor = source.cursor_for(&peer_site).await?;
    loop {
        stream
            .send(&SyncMessage::Pull {
                after: cursor,
                exclude_site: site_id.clone(),
                max_bytes: DEFAULT_MAX_BYTES,
            })
            .await?;

        match stream.receive().await? {
            SyncMessage::Changes { page } => {
                if page.next.is_strictly_after(cursor) {
                    if !page.changes.is_empty() {
                        source.apply(&peer_site, &page).await?;
                        applied += page.changes.len();
                    }
                } else if page.next != cursor || page.more || !page.changes.is_empty() {
                    return Err(AgentError::Session(
                        *peer_id,
                        format!("peer returned a non-advancing cursor after {cursor:?}"),
                    ));
                }
                cursor = page.next;
                if !page.more {
                    break;
                }
            }
            SyncMessage::Error { message } => {
                return Err(AgentError::Session(*peer_id, message));
            }
            other => {
                return Err(AgentError::Session(
                    *peer_id,
                    format!("expected Changes, got {other:?}"),
                ));
            }
        }
    }

    // 3. Ack: we applied through `cursor`; the peer records it as served.
    //    Then half-close the stream (send-finished) so the server's reader
    //    sees the session end. The EncryptedStream borrows `send`/`recv`,
    //    so finish the raw stream after dropping it.
    let outcome = {
        stream
            .send(&SyncMessage::Ack {
                applied_through: cursor,
            })
            .await?;
        PeerSyncOutcome { applied, cursor }
    };
    drop(stream);
    let _ = send.finish();

    Ok(outcome)
}

/// Serve one inbound bi-stream as **server**: the same protocol, mirrored.
/// Reads the client's Hello → answers with our Hello (Error on schema
/// mismatch) → serves Pull pages → on Ack records the served cursor for
/// that peer and ends the session.
///
/// Returns the outcome from the server side (rows served, final cursor
/// served through) for tests and diagnostics.
async fn serve_sync_stream(
    stream: &mut EncryptedStream<'_, '_, SendStream, RecvStream>,
    source: &dyn ChangeSource,
    peer_id: &PublicKey,
) -> Result<PeerSyncOutcome, AgentError> {
    let (peer_site, client_schema) = match stream.receive().await? {
        SyncMessage::Hello {
            site_id,
            schema_version,
            ..
        } if !site_id.is_empty() => (site_id, schema_version),
        SyncMessage::Hello { .. } => {
            return Err(AgentError::Session(
                *peer_id,
                "peer sent an empty site id".into(),
            ));
        }
        SyncMessage::Error { message } => {
            return Err(AgentError::Session(*peer_id, message));
        }
        other => {
            return Err(AgentError::Session(
                *peer_id,
                format!("expected Hello, got {other:?}"),
            ));
        }
    };

    let my_site = source.site_id().await?;
    let my_schema = source.schema_version().await?;
    if client_schema != my_schema {
        let message = format!("schema version mismatch: ours {my_schema}, peer {client_schema}");
        let _ = stream
            .send(&SyncMessage::Error {
                message: message.clone(),
            })
            .await;
        return Err(AgentError::Session(*peer_id, message));
    }

    let db_version = source.db_version().await?;
    stream
        .send(&SyncMessage::Hello {
            site_id: my_site.clone(),
            db_version,
            schema_version: my_schema,
        })
        .await?;

    // Serve Pull pages until the client Acks.
    let mut served = 0usize;
    loop {
        match stream.receive().await? {
            SyncMessage::Pull {
                after,
                exclude_site,
                max_bytes,
            } => {
                let page = source
                    .pull(after, &exclude_site, max_bytes.min(DEFAULT_MAX_BYTES))
                    .await?;
                served += page.changes.len();
                stream.send(&SyncMessage::Changes { page }).await?;
            }
            SyncMessage::Ack { applied_through } => {
                source.record_served(&peer_site, applied_through).await?;
                return Ok(PeerSyncOutcome {
                    applied: served,
                    cursor: applied_through,
                });
            }
            SyncMessage::Error { message } => {
                return Err(AgentError::Session(*peer_id, message));
            }
            other => {
                return Err(AgentError::Session(
                    *peer_id,
                    format!("expected Pull or Ack, got {other:?}"),
                ));
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Encrypted stream (whole-frame AEAD over one bi-stream)
// ---------------------------------------------------------------------------

/// A bi-stream wrapper that encrypts every whole frame with the per-peer
/// symmetric key before writing, and decrypts on read — protocol v2's
/// "whole-frame encrypted" contract, replacing v1's field-level encryption
/// of `body`/`blob` payloads only.
struct EncryptedStream<'a, 'b, W, R> {
    secret: &'a iroh::SecretKey,
    peer_id: &'a PublicKey,
    writer: &'b mut W,
    reader: &'b mut R,
}

impl<'a, 'b, W, R> EncryptedStream<'a, 'b, W, R>
where
    W: AsyncWrite + Unpin,
    R: AsyncRead + Unpin,
{
    fn new(
        secret: &'a iroh::SecretKey,
        peer_id: &'a PublicKey,
        writer: &'b mut W,
        reader: &'b mut R,
    ) -> Self {
        Self {
            secret,
            peer_id,
            writer,
            reader,
        }
    }

    /// Serialize, encrypt, frame, write.
    async fn send(&mut self, message: &SyncMessage) -> Result<(), std::io::Error> {
        let json = serde_json::to_vec(message)?;
        let sealed = crypto::encrypt(self.secret, self.peer_id, &json)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        let len = (sealed.len() as u32).to_be_bytes();
        self.writer.write_all(&len).await?;
        self.writer.write_all(&sealed).await?;
        self.writer.flush().await?;
        Ok(())
    }

    /// Read, deframe, decrypt, deserialize.
    async fn receive(&mut self) -> Result<SyncMessage, std::io::Error> {
        let mut len_buf = [0u8; 4];
        self.reader.read_exact(&mut len_buf).await?;
        let len = u32::from_be_bytes(len_buf) as usize;
        if len > MAX_FRAME_BYTES {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("frame too large: {len} bytes"),
            ));
        }
        let mut sealed = vec![0u8; len];
        self.reader.read_exact(&mut sealed).await?;
        let json = crypto::decrypt(self.secret, self.peer_id, &sealed)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        serde_json::from_slice(&json)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
    }
}

// ---------------------------------------------------------------------------
// iroh dial (outbound to a peer)
// ---------------------------------------------------------------------------

/// Resolve `node_id` to an [`EndpointAddr`] and dial it. In `Loopback` mode
/// the peer's socket addresses come from the process-local `DIRECT_ADDRS`
/// registry; in `Discovered` mode iroh resolves the node id via
/// relay/DNS-pkarr. Allowlist enforcement is the caller's first duty.
async fn dial_peer(
    node_id: &PublicKey,
    endpoint: &Endpoint,
    transport: AgentTransport,
) -> Result<iroh::endpoint::Connection, AgentError> {
    let mut ea = EndpointAddr::new(*node_id);
    if transport == AgentTransport::Loopback {
        let addrs = lookup_direct_addrs(node_id).await;
        for a in addrs {
            ea = ea.with_ip_addr(a);
        }
    }

    // Retry with backoff: a peer may not be dialable on the very first
    // attempt for reasons that differ by transport, and a bounded ladder
    // rides that out without blocking a real sync round forever on a
    // genuinely offline peer.
    //
    // FINDING (carried from v1): `Endpoint::connect` has no cap of its own. A
    // peer bound to a dead address never sends anything back, so iroh keeps
    // waiting up to its own internal handshake/idle timeout — tens of
    // seconds. Each attempt is wrapped in its own `tokio::time::timeout` so
    // a non-responding peer fails on this ladder's schedule, not iroh's.
    let mut last_err = String::from("dial failed");
    for attempt in 0..dial_attempts(transport) {
        match tokio::time::timeout(
            dial_attempt_timeout(transport),
            endpoint.connect(ea.clone(), SYNC_ALPN),
        )
        .await
        {
            Ok(Ok(conn)) => return Ok(conn),
            Ok(Err(e)) => last_err = format!("{e}"),
            Err(_) => last_err = "dial attempt timed out".to_string(),
        }
        tokio::time::sleep(dial_backoff(transport, attempt)).await;
    }
    Err(AgentError::Session(*node_id, format!("dial: {last_err}")))
}

/// Number of dial attempts before `dial_peer` gives up. See [`dial_backoff`]
/// for the per-attempt wait and why the two modes differ.
fn dial_attempts(transport: AgentTransport) -> u32 {
    match transport {
        AgentTransport::Loopback => 8,
        AgentTransport::Discovered => 6,
    }
}

/// Per-attempt backoff before the next `connect()` retry.
///
/// `Loopback`: 1, 2, 4, …, 128ms (~255ms total across 8 attempts) — only
/// ever rides out a same-process peer endpoint still finishing its own
/// `bind()` under concurrent test load.
///
/// `Discovered`: 250ms, 500ms, 1s, 2s, 4s, 8s (~15.75s total across 6
/// attempts) — room for a DNS/pkarr lookup plus a relay-assisted QUIC
/// handshake on a slow-but-alive path, bounded on a genuinely dead one.
fn dial_backoff(transport: AgentTransport, attempt: u32) -> std::time::Duration {
    let base_ms: u64 = match transport {
        AgentTransport::Loopback => 1,
        AgentTransport::Discovered => 250,
    };
    std::time::Duration::from_millis(base_ms << attempt)
}

/// Cap on a single `connect()` attempt. `Loopback`: 500ms bounds a dead-port
/// dial while staying generous for a same-machine handshake.
/// `Discovered`: 8s allows a slow DNS/relay path without letting iroh's
/// internal multi-ten-second timeout dominate.
fn dial_attempt_timeout(transport: AgentTransport) -> std::time::Duration {
    match transport {
        AgentTransport::Loopback => std::time::Duration::from_millis(500),
        AgentTransport::Discovered => std::time::Duration::from_secs(8),
    }
}

// ---------------------------------------------------------------------------
// iroh inbound accept loop
// ---------------------------------------------------------------------------

/// Accept inbound iroh connections from peers. Each connection's remote
/// `EndpointId` is checked against the allowlist; non-allowlisted peers are
/// refused. Allowed peers' bi-streams are served as protocol v2 **server**
/// sessions. Enforcement shape unchanged from v1, including the per-stream
/// revocation re-check.
async fn accept_iroh(
    endpoint: Endpoint,
    peers: PeerStore,
    source: Arc<dyn ChangeSource>,
    identity: Identity,
) {
    loop {
        let Some(incoming) = endpoint.accept().await else {
            break;
        };
        let peers_refuse = peers.clone();
        let source = Arc::clone(&source);
        let identity = identity.clone();
        tokio::spawn(async move {
            // Await `Incoming` to complete the handshake and yield the
            // authenticated `Connection` (iroh authenticates the peer's public
            // key during the handshake — `remote_id()` is verified, not
            // self-asserted; `Incoming` exposes only `remote_addr`
            // pre-handshake, so the id check is post-handshake by necessity).
            let conn = match incoming.await {
                Ok(c) => c,
                Err(_) => return,
            };
            let remote = conn.remote_id();
            if !peers_refuse.is_allowed(&remote) {
                // Not allowlisted: tear down without serving.
                conn.close(iroh::endpoint::VarInt::from_u32(1), b"not allowlisted");
                return;
            }
            peers_refuse.touch_last_seen(&remote);
            // Serve bi-streams from this connection until it closes.
            //
            // The allowlist MUST be re-checked per stream, not only once per
            // connection: a peer revoked while its QUIC connection is still
            // open must stop being served immediately — that is the case
            // revocation exists for.
            while let Ok((mut send, mut recv)) = conn.accept_bi().await {
                if !peers_refuse.is_allowed(&remote) {
                    conn.close(iroh::endpoint::VarInt::from_u32(1), b"revoked");
                    break;
                }
                let source = Arc::clone(&source);
                let secret = identity.secret_key().clone();
                let remote_id = remote;
                tokio::spawn(async move {
                    // One bi-stream = one protocol v2 server session. A
                    // session error is logged and the stream ends — the
                    // client sees a closed stream, never a hang. Bounded so
                    // a stuck client cannot pin a task forever.
                    let result = tokio::time::timeout(std::time::Duration::from_secs(120), async {
                        let mut stream =
                            EncryptedStream::new(&secret, &remote_id, &mut send, &mut recv);
                        serve_sync_stream(&mut stream, &*source, &remote_id).await
                    })
                    .await;
                    let _ = send.finish();
                    match result {
                        Ok(Ok(outcome)) => {
                            tracing::debug!(
                                "sync-p2p: served peer {remote_id}: {} changes through {:?}",
                                outcome.applied,
                                outcome.cursor
                            );
                        }
                        Ok(Err(e)) => {
                            tracing::warn!("sync-p2p: session with peer {remote_id} failed: {e}");
                        }
                        Err(_) => {
                            tracing::warn!("sync-p2p: session with peer {remote_id} timed out");
                        }
                    }
                });
            }
        });
    }
}

// ---------------------------------------------------------------------------
// peer direct-address registry (Loopback mode only)
// ---------------------------------------------------------------------------

// A process-local registry mapping a peer node id → the iroh socket addresses
// it is reachable on. Populated directly by tests (all agents live in the
// same process). Only consulted in `AgentTransport::Loopback`.

static DIRECT_ADDRS: tokio::sync::Mutex<
    Option<std::collections::HashMap<[u8; 32], Vec<std::net::SocketAddr>>>,
> = tokio::sync::Mutex::const_new(None);

async fn direct_addrs_map() -> tokio::sync::MutexGuard<
    'static,
    Option<std::collections::HashMap<[u8; 32], Vec<std::net::SocketAddr>>>,
> {
    let mut g = DIRECT_ADDRS.lock().await;
    if g.is_none() {
        *g = Some(std::collections::HashMap::new());
    }
    g
}

/// Register a peer's iroh direct addresses so a `Loopback`-mode agent can
/// dial it without discovery. (Same-machine proof / tests only.)
pub async fn register_direct_addr(node_id: PublicKey, addrs: Vec<std::net::SocketAddr>) {
    let mut map = direct_addrs_map().await;
    map.as_mut().unwrap().insert(*node_id.as_bytes(), addrs);
}

/// Look up a peer's registered direct addresses. `.await`s the lock — a
/// `try_lock` + empty fallback produced spurious dial failures under
/// contention (a v1 finding, kept fixed here).
async fn lookup_direct_addrs(node_id: &PublicKey) -> Vec<std::net::SocketAddr> {
    let g = DIRECT_ADDRS.lock().await;
    g.as_ref()
        .and_then(|m| m.get(node_id.as_bytes()))
        .cloned()
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// REGRESSION (carried from v1): `lookup_direct_addrs` must wait for the
    /// lock, not fall back to an empty list under contention (which produced
    /// spurious dial failures).
    #[tokio::test]
    async fn lookup_direct_addrs_awaits_the_lock_instead_of_dropping_addrs_under_contention() {
        let node_id = Identity::for_test().id();
        let addr: std::net::SocketAddr = "127.0.0.1:9".parse().unwrap();
        register_direct_addr(node_id, vec![addr]).await;

        let held = tokio::spawn(async {
            let g = DIRECT_ADDRS.lock().await;
            tokio::time::sleep(std::time::Duration::from_millis(150)).await;
            drop(g);
        });
        // Give the background task a chance to grab the lock first.
        tokio::time::sleep(std::time::Duration::from_millis(30)).await;

        let found = lookup_direct_addrs(&node_id).await;
        held.await.unwrap();

        assert_eq!(
            found,
            vec![addr],
            "lookup must wait for the lock and return the registered address"
        );
    }
}

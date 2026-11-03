//! The P2P agent: owns the iroh endpoint, accepts and dials sync sessions, and
//! runs the periodic sync tick.
//!
//! The agent moves [`WireChange`](crate::change::WireChange)s between a
//! [`ChangeSource`] (implemented by `plugins/db` over cr-sqlite) and allowlisted
//! peers, using protocol v2 ([`crate::protocol`]) on ALPN [`SYNC_ALPN`]. All
//! per-peer logic lives in [`crate::session`]; this module is connection
//! management:
//!
//! * **dial** ([`P2pAgent::sync_with`], and the tick) — allowlist check, dial
//!   with a bounded retry ladder, open one bi-stream, run a
//!   [`SyncSession`] as [`Role::Dialer`];
//! * **accept** — complete the handshake, check the authenticated remote id
//!   against the allowlist, then serve bi-streams as [`Role::Acceptor`];
//! * **tick** — every [`AgentConfig::tick_interval`] (default 60 s), sync with
//!   every allowlisted peer concurrently. Unreachable peers fail after the dial
//!   ladder and are retried next tick.
//!
//! ## Allowlist enforcement (closes the §12 SSRF finding)
//!
//! Every outbound dial and every inbound connection is checked against the
//! [`PeerStore`]; a node id not on the allowlist is refused in *both*
//! directions. The inbound check is repeated per stream so a peer revoked
//! while its QUIC connection is still open stops being served.

use std::collections::HashMap;
use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;

use iroh::{Endpoint, EndpointAddr, PublicKey, RelayMode};
use tokio::sync::Mutex;
use tokio::task::JoinSet;

use crate::change::ChangeSource;
use crate::identity::Identity;
use crate::pairing::{
    JoinMsg, PAIRING_ALPN, PairingOffer, PairingService, RejectMsg, compute_proof, compute_sas,
};
use crate::peers::PeerStore;
pub use crate::protocol::SYNC_ALPN;
use crate::protocol::{PairingReply, Role, read_json_frame, write_json_frame};
use crate::session::{SessionConfig, SessionError, SessionReport, SyncSession};

/// How often the agent syncs with every allowlisted peer by default.
pub const DEFAULT_TICK_INTERVAL: Duration = Duration::from_secs(60);

/// Errors from starting or running the P2P agent.
#[derive(Debug, thiserror::Error)]
pub enum AgentError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Iroh(#[from] iroh::endpoint::BindError),
    #[error("invalid bind address")]
    BadBindAddr,
    #[error("no platform data directory available")]
    NoDataDir,
    #[error(transparent)]
    Identity(#[from] crate::identity::IdentityError),
    #[error(transparent)]
    Peers(#[from] crate::peers::PeersError),
    #[error("peer is not allowlisted")]
    NotAllowlisted,
    #[error("cannot sync with this device itself")]
    SelfPeer,
    #[error("dial peer: {0}")]
    Dial(String),
    #[error("open stream: {0}")]
    Stream(String),
    #[error(transparent)]
    Session(#[from] SessionError),
    #[error("pairing: {0}")]
    Pairing(String),
    #[error("tick interval must be non-zero")]
    ZeroTickInterval,
    #[error("sync task failed: {0}")]
    Task(String),
}

/// How the agent's iroh endpoint finds and is found by peers. See
/// `docs/internal/sync-p2p.md` §20 for the full rationale.
///
/// This is deliberately an explicit enum rather than a bool: the two modes
/// differ in more than one axis (relay, bind address, address-lookup
/// publishing) and a bool would invite someone to thread only one of those
/// through later and silently produce a hybrid that is neither offline-safe
/// nor really discoverable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentTransport {
    /// Same-machine proof / test behavior: `RelayMode::Disabled`, bound to
    /// `127.0.0.1` only, peer addresses resolved from the process-local
    /// `DIRECT_ADDRS` registry populated by [`register_direct_addr`].
    /// Deterministic and fully offline — what every test uses.
    Loopback,
    /// Production: `RelayMode::Default`, bound on all interfaces (dual-stack —
    /// the default when no explicit `bind_addr` is set), and iroh's DNS/pkarr
    /// address-lookup service enabled so the endpoint both publishes its own
    /// address record and resolves peers by node id alone. Reaches n0's
    /// infrastructure on the open internet — see §20 before assuming this is
    /// free of privacy characteristics worth documenting.
    Discovered,
}

/// Agent configuration. `Default` is the production setup.
#[derive(Debug, Clone)]
pub struct AgentConfig {
    pub transport: AgentTransport,
    /// Period of the all-peers sync tick; `None` disables the tick (sessions
    /// then only happen via [`P2pAgent::sync_with`] / inbound dials). Must be
    /// non-zero.
    pub tick_interval: Option<Duration>,
    pub session: SessionConfig,
    /// This device's display name, sent to the other side during pairing.
    pub device_name: String,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            transport: AgentTransport::Discovered,
            tick_interval: Some(DEFAULT_TICK_INTERVAL),
            session: SessionConfig::default(),
            device_name: String::new(),
        }
    }
}

/// What a successful [`P2pAgent::pair_with`] learned.
#[derive(Debug, Clone)]
pub struct PairingOutcome {
    /// The offerer's identity and display name.
    pub offerer: crate::pairing::AcceptMsg,
    /// Short authentication string to compare with the offerer's screen.
    pub sas: String,
}

struct Inner<S: ChangeSource> {
    endpoint: Endpoint,
    identity: Identity,
    peers: PeerStore,
    source: Arc<S>,
    config: AgentConfig,
    /// One apply/cursor lock per peer, shared by every session with it.
    apply_locks: StdMutex<HashMap<PublicKey, Arc<Mutex<()>>>>,
    pairing: PairingService,
}

/// A running P2P agent.
pub struct P2pAgent<S: ChangeSource> {
    inner: Arc<Inner<S>>,
    accept_handle: tokio::task::JoinHandle<()>,
    tick_handle: Option<tokio::task::JoinHandle<()>>,
}

impl<S: ChangeSource> P2pAgent<S> {
    /// Start an agent using the persisted identity + allowlist from the app
    /// data dir (`<data_dir>/notare/sync/`), with real relay/DNS discovery
    /// enabled (`AgentTransport::Discovered`). This is the app entry point.
    pub async fn start(source: Arc<S>) -> Result<Self, AgentError> {
        let identity = Identity::load_or_create()?;
        let peers = PeerStore::load_or_create()?;
        Self::start_with_config(identity, peers, source, AgentConfig::default()).await
    }

    /// Start with an explicit identity + peer store on `AgentTransport::Loopback`
    /// (same-machine, deterministic, offline) with the default tick. Tests use
    /// [`Self::start_with_config`] to turn the tick off.
    pub async fn start_with(
        identity: Identity,
        peers: PeerStore,
        source: Arc<S>,
    ) -> Result<Self, AgentError> {
        let config = AgentConfig {
            transport: AgentTransport::Loopback,
            ..AgentConfig::default()
        };
        Self::start_with_config(identity, peers, source, config).await
    }

    /// Start with an explicit identity, peer store, and configuration.
    pub async fn start_with_config(
        identity: Identity,
        peers: PeerStore,
        source: Arc<S>,
        config: AgentConfig,
    ) -> Result<Self, AgentError> {
        if config.tick_interval.is_some_and(|d| d.is_zero()) {
            return Err(AgentError::ZeroTickInterval);
        }
        // The secret key IS the device identity key — iroh's NodeId/EndpointId
        // derives from it — in both modes.
        let endpoint = match config.transport {
            // Disable relay for the same-machine proof (deterministic, no
            // external network), bind localhost only.
            AgentTransport::Loopback => {
                Endpoint::builder(iroh::endpoint::presets::Minimal)
                    .secret_key(identity.secret_key().clone())
                    .alpns(vec![SYNC_ALPN.to_vec(), PAIRING_ALPN.to_vec()])
                    .relay_mode(RelayMode::Disabled)
                    .bind_addr("127.0.0.1:0")
                    .map_err(|_| AgentError::BadBindAddr)?
                    .bind()
                    .await?
            }
            // `presets::N0` wires up the n0 DNS/pkarr address-lookup service
            // (publish + resolve) and n0's relay servers — confirmed against
            // the vendored iroh 1.1.0 source
            // (`iroh-1.1.0/src/endpoint/presets.rs`; discovery was renamed
            // `address_lookup` in this version, so `presets::Minimal` — which
            // sets nothing but the TLS crypto provider — does NOT include it).
            // `relay_mode` is set explicitly afterward rather than trusting
            // the preset's `default_relay_mode()`, so this endpoint is
            // `RelayMode::Default` even if `IROH_FORCE_STAGING_RELAYS` is set
            // in the environment. No `bind_addr` call: the builder's
            // unspecified defaults (`0.0.0.0` + `[::]`) bind dual-stack on
            // every interface, which is what "do not hardcode IPv4" requires.
            AgentTransport::Discovered => {
                Endpoint::builder(iroh::endpoint::presets::N0)
                    .secret_key(identity.secret_key().clone())
                    .alpns(vec![SYNC_ALPN.to_vec(), PAIRING_ALPN.to_vec()])
                    .relay_mode(RelayMode::Default)
                    .bind()
                    .await?
            }
        };

        let tick_interval = config.tick_interval;
        let inner = Arc::new(Inner {
            endpoint,
            identity,
            peers,
            source,
            config,
            apply_locks: StdMutex::new(HashMap::new()),
            pairing: PairingService::new(),
        });

        let accept_handle = tokio::spawn(accept_loop(Arc::clone(&inner)));
        let tick_handle =
            tick_interval.map(|every| tokio::spawn(tick_loop(Arc::clone(&inner), every)));

        Ok(Self {
            inner,
            accept_handle,
            tick_handle,
        })
    }

    /// This device's node id / iroh EndpointId.
    pub fn node_id(&self) -> PublicKey {
        self.inner.identity.id()
    }

    /// The peer allowlist (for adding/removing peers at runtime).
    pub fn peers(&self) -> &PeerStore {
        &self.inner.peers
    }

    /// Pairing state (offers, pending SAS confirmations) for the plugin layer.
    pub fn pairing(&self) -> &PairingService {
        &self.inner.pairing
    }

    /// Join the pairing offer in `offer` (scanned/pasted by the user): dial the
    /// offerer on [`PAIRING_ALPN`] and prove knowledge of the offer secret.
    pub async fn pair_with(&self, offer: &PairingOffer) -> Result<PairingOutcome, AgentError> {
        self.inner.pair_with(offer).await
    }

    /// The iroh endpoint's bound direct addresses (so a peer can dial us with
    /// a direct address rather than via relay — used by tests to build the dial
    /// address without external discovery).
    pub fn direct_addresses(&self) -> Vec<std::net::SocketAddr> {
        self.inner.endpoint.bound_sockets()
    }

    /// Run one sync session with `peer` now: pull its changes and serve ours.
    /// Refused before any dial if `peer` is not allowlisted.
    pub async fn sync_with(&self, peer: &PublicKey) -> Result<SessionReport, AgentError> {
        self.inner.sync_with(peer).await
    }

    /// Sync with every allowlisted peer concurrently (what the tick does).
    pub async fn sync_all(&self) -> Vec<(PublicKey, Result<SessionReport, AgentError>)> {
        self.inner.sync_all().await
    }

    /// Gracefully stop the agent.
    pub async fn stop(self) {
        self.accept_handle.abort();
        if let Some(t) = self.tick_handle {
            t.abort();
        }
        self.inner.endpoint.close().await;
    }
}

impl<S: ChangeSource> Inner<S> {
    fn apply_lock(&self, peer: &PublicKey) -> Arc<Mutex<()>> {
        let mut map = self.apply_locks.lock().unwrap_or_else(|e| e.into_inner());
        Arc::clone(map.entry(*peer).or_default())
    }

    fn session(&self, peer: PublicKey, role: Role) -> SyncSession<S> {
        SyncSession::new(
            Arc::clone(&self.source),
            self.identity.secret_key().clone(),
            peer,
            role,
            self.config.session.clone(),
        )
        .with_apply_lock(self.apply_lock(&peer))
    }

    async fn sync_with(&self, peer: &PublicKey) -> Result<SessionReport, AgentError> {
        if *peer == self.identity.id() {
            return Err(AgentError::SelfPeer);
        }
        // ALLOWLIST ENFORCEMENT (outbound): refuse to dial a non-allowlisted
        // node id. Closes the §12 SSRF finding — only a paired peer is dialed.
        if !self.peers.is_allowed(peer) {
            return Err(AgentError::NotAllowlisted);
        }
        let conn = self
            .dial_peer(peer, SYNC_ALPN)
            .await
            .map_err(AgentError::Dial)?;
        let (send, recv) = match conn.open_bi().await {
            Ok(s) => s,
            Err(e) => {
                conn.close(iroh::endpoint::VarInt::from_u32(1), b"open stream failed");
                return Err(AgentError::Stream(e.to_string()));
            }
        };
        let result = self.session(*peer, Role::Dialer).run(recv, send).await;
        conn.close(iroh::endpoint::VarInt::from_u32(0), b"done");
        if result.is_ok() {
            self.peers.touch_last_seen(peer);
        }
        Ok(result?)
    }

    async fn sync_all(self: &Arc<Self>) -> Vec<(PublicKey, Result<SessionReport, AgentError>)> {
        let mut set = JoinSet::new();
        let mut task_peers = HashMap::new();
        for peer in self.peers.list_peers() {
            let inner = Arc::clone(self);
            let id = peer.node_id;
            let handle = set.spawn(async move { (id, inner.sync_with(&id).await) });
            task_peers.insert(handle.id(), id);
        }
        let mut out = Vec::new();
        while let Some(joined) = set.join_next().await {
            match joined {
                Ok(r) => out.push(r),
                Err(e) => {
                    if let Some(peer) = task_peers.get(&e.id()) {
                        out.push((*peer, Err(AgentError::Task(e.to_string()))));
                    }
                }
            }
        }
        out
    }

    /// Resolve `node_id` to an [`EndpointAddr`] and dial it. In `Loopback` mode the
    /// process-local `DIRECT_ADDRS` registry supplies the peer's socket addresses
    /// directly — there is no relay and no address lookup to fall back on. In
    /// `Discovered` mode no addresses are injected: `EndpointAddr::new(node_id)`
    /// names the peer by node id alone, and iroh's relay/DNS address-lookup
    /// service (wired up in [`P2pAgent::start_with_config`]) resolves it (SYNC-8).
    async fn dial_peer(
        &self,
        node_id: &PublicKey,
        alpn: &'static [u8],
    ) -> Result<iroh::endpoint::Connection, String> {
        let transport = self.config.transport;
        let mut ea = EndpointAddr::new(*node_id);
        if transport == AgentTransport::Loopback {
            for a in lookup_direct_addrs(node_id).await {
                ea = ea.with_ip_addr(a);
            }
        }

        // Retry with backoff: a peer may not be dialable on the very first
        // attempt for reasons that differ by transport (see `dial_attempts` /
        // `dial_backoff`), and a bounded ladder rides that out without blocking a
        // real sync call forever on a genuinely offline peer.
        //
        // FINDING (SYNC-8, found while writing the offline-reconnect test):
        // `Endpoint::connect` has no cap of its own. A peer that is offline in the
        // way that matters here — bound to a dead address, not merely slow — never
        // sends anything back, so there is no QUIC packet to fail fast on; iroh
        // just keeps waiting up to its own internal handshake/idle timeout, which
        // is tens of seconds. Retried 8x that turns "offline peer" into a
        // multi-minute hang. Each attempt is therefore wrapped in its own
        // `tokio::time::timeout` so a non-responding peer fails on this ladder's
        // schedule, not iroh's.
        let mut last_err = String::from("dial failed");
        for attempt in 0..dial_attempts(transport) {
            match tokio::time::timeout(
                dial_attempt_timeout(transport),
                self.endpoint.connect(ea.clone(), alpn),
            )
            .await
            {
                Ok(Ok(conn)) => return Ok(conn),
                Ok(Err(e)) => last_err = format!("{e}"),
                Err(_) => last_err = "dial attempt timed out".to_string(),
            }
            tokio::time::sleep(dial_backoff(transport, attempt)).await;
        }
        Err(last_err)
    }
}

impl<S: ChangeSource> Inner<S> {
    /// Offerer side of pairing, on a connection that negotiated
    /// [`PAIRING_ALPN`]. Unlike sync this is open to peers that are not on the
    /// allowlist — that is its purpose — so it is gated on an unexpired offer
    /// and on the join proof instead. Exactly one `JoinMsg` is read and one
    /// [`PairingReply`] written; adding the peer to the allowlist is left to the
    /// user's SAS confirmation (`PairingService::confirm`).
    async fn serve_pairing(&self, conn: iroh::endpoint::Connection) {
        const STEP: Duration = Duration::from_secs(10);
        let remote = conn.remote_id();
        if !self.pairing.has_active_offer() {
            conn.close(iroh::endpoint::VarInt::from_u32(1), b"no active offer");
            return;
        }
        let Ok(Ok((mut send, mut recv))) = tokio::time::timeout(STEP, conn.accept_bi()).await
        else {
            conn.close(iroh::endpoint::VarInt::from_u32(1), b"no join");
            return;
        };
        let join: JoinMsg = match tokio::time::timeout(STEP, read_json_frame(&mut recv)).await {
            Ok(Ok(j)) => j,
            _ => {
                conn.close(iroh::endpoint::VarInt::from_u32(1), b"bad join");
                return;
            }
        };
        let reply = if join.device_id != remote.to_z32() {
            // The id in the message is self-asserted; the one on the connection is
            // authenticated by the TLS handshake. They must agree.
            PairingReply::Reject(RejectMsg {
                reason: "device id does not match the connection".into(),
            })
        } else {
            match self
                .pairing
                .handle_join(&join, &|id| self.peers.is_revoked(id))
            {
                Ok((accept, _sas)) => PairingReply::Accept(accept),
                Err(e) => PairingReply::Reject(RejectMsg {
                    reason: e.to_string(),
                }),
            }
        };
        if write_json_frame(&mut send, &reply).await.is_ok() {
            let _ = send.finish();
            // Let the joiner read the reply before the connection goes away.
            let _ = tokio::time::timeout(STEP, conn.closed()).await;
        }
        conn.close(iroh::endpoint::VarInt::from_u32(0), b"done");
    }

    /// Joiner side of pairing: prove knowledge of the offer secret to the
    /// offerer named in `offer`. On success returns the offerer's details and
    /// the SAS code to show the user; the offerer still has to confirm.
    async fn pair_with(&self, offer: &PairingOffer) -> Result<PairingOutcome, AgentError> {
        const STEP: Duration = Duration::from_secs(10);
        if offer.is_expired() {
            return Err(AgentError::Pairing("offer has expired".into()));
        }
        let offerer = offer
            .offerer_id()
            .map_err(|e| AgentError::Pairing(e.to_string()))?;
        if offerer == self.identity.id() {
            return Err(AgentError::SelfPeer);
        }
        let own_id = self.identity.id().to_z32();
        let offerer_z32 = offerer.to_z32();
        let conn = self
            .dial_peer(&offerer, PAIRING_ALPN)
            .await
            .map_err(AgentError::Dial)?;
        let result = async {
            let (mut send, mut recv) = conn
                .open_bi()
                .await
                .map_err(|e| AgentError::Stream(e.to_string()))?;
            let join = JoinMsg {
                device_id: own_id.clone(),
                name: self.config.device_name.clone(),
                proof: compute_proof(&offer.secret, &own_id, &offerer_z32),
            };
            let io_err = |e: std::io::Error| AgentError::Pairing(e.to_string());
            write_json_frame(&mut send, &join).await.map_err(io_err)?;
            let _ = send.finish();
            let reply: PairingReply = tokio::time::timeout(STEP, read_json_frame(&mut recv))
                .await
                .map_err(|_| AgentError::Pairing("timed out waiting for the offerer".into()))?
                .map_err(io_err)?;
            match reply {
                PairingReply::Accept(accept) => {
                    // The authenticated connection is the offerer we dialed;
                    // anything else claiming to accept is not to be trusted.
                    if accept.device_id != offerer_z32 {
                        return Err(AgentError::Pairing(
                            "accept names a different device than the one dialed".into(),
                        ));
                    }
                    let sas = compute_sas(&own_id, &offerer_z32, &offer.secret);
                    Ok(PairingOutcome {
                        offerer: accept,
                        sas,
                    })
                }
                PairingReply::Reject(r) => Err(AgentError::Pairing(r.reason)),
            }
        }
        .await;
        conn.close(iroh::endpoint::VarInt::from_u32(0), b"done");
        result
    }
}

/// Number of dial attempts before `dial_peer` gives up. See [`dial_backoff`]
/// for the per-attempt wait and why the two modes differ.
fn dial_attempts(transport: AgentTransport) -> u32 {
    match transport {
        AgentTransport::Loopback => 8,
        AgentTransport::Discovered => 6,
    }
}

/// Per-attempt backoff before the next `connect()` retry in `dial_peer`.
///
/// `Loopback`: 1, 2, 4, …, 128ms (~255ms total across 8 attempts). That ladder
/// only ever needs to ride out a same-process peer endpoint that is still
/// finishing its own `bind()` under concurrent test load; the peer is either up
/// within a couple hundred ms or it never will be.
///
/// `Discovered`: retuned for a real relayed QUIC dial. Over a real network a
/// `connect()` may need to complete a DNS/pkarr address-lookup round trip
/// and/or a relay handshake before the QUIC handshake can even start, which
/// routinely costs hundreds of ms to low seconds even on a healthy path. 250ms,
/// 500ms, 1s, 2s, 4s, 8s (~15.75s total across 6 attempts) gives a WAN dial
/// room to complete without turning a genuinely offline peer into a
/// multi-minute hang — bounded, not unbounded.
fn dial_backoff(transport: AgentTransport, attempt: u32) -> Duration {
    let base_ms: u64 = match transport {
        AgentTransport::Loopback => 1,
        AgentTransport::Discovered => 250,
    };
    Duration::from_millis(base_ms << attempt)
}

/// Cap on a single `connect()` attempt in `dial_peer`. See the `FINDING` note
/// at the call site.
///
/// `Loopback`: 500ms is generous for a same-machine handshake (normally single
/// digit ms) while still bounding a dead-port dial. `Discovered`: 8s allows
/// room for a real DNS/pkarr address-lookup round trip plus a relay-assisted
/// QUIC handshake on a path that is merely slow, not offline.
/// AUDIT (auditor, minimax, 2026-08-31): an earlier 5s cap was tight enough
/// that a *reachable but slow* WAN path could be misread as offline; widened to
/// 8s.
fn dial_attempt_timeout(transport: AgentTransport) -> Duration {
    match transport {
        AgentTransport::Loopback => Duration::from_millis(500),
        AgentTransport::Discovered => Duration::from_secs(8),
    }
}

// ---------------------------------------------------------------------------
// tick
// ---------------------------------------------------------------------------

async fn tick_loop<S: ChangeSource>(inner: Arc<Inner<S>>, every: Duration) {
    let mut interval = tokio::time::interval(every);
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        interval.tick().await;
        for (peer, result) in inner.sync_all().await {
            match result {
                Ok(r) => tracing::debug!(
                    "sync-p2p: synced with {}: pulled {} served {}",
                    peer.fmt_short(),
                    r.changes_pulled,
                    r.changes_served
                ),
                // An offline peer is the normal case for a laptop; not a warning.
                Err(AgentError::Dial(e)) => {
                    tracing::debug!("sync-p2p: {} unreachable: {e}", peer.fmt_short())
                }
                Err(e) => tracing::warn!("sync-p2p: sync with {} failed: {e}", peer.fmt_short()),
            }
        }
    }
}

// ---------------------------------------------------------------------------
// iroh inbound accept loop
// ---------------------------------------------------------------------------

/// Accept inbound iroh connections from peers. Each connection's remote
/// `EndpointId` is checked against the allowlist; non-allowlisted peers are
/// refused. This is the inbound half of the §12 SSRF fix.
async fn accept_loop<S: ChangeSource>(inner: Arc<Inner<S>>) {
    loop {
        // `endpoint.accept().await` yields `Option<Incoming>` — None once the
        // endpoint is closed.
        let Some(incoming) = inner.endpoint.accept().await else {
            break;
        };
        let inner = Arc::clone(&inner);
        // ALLOWLIST ENFORCEMENT (inbound): we must complete the TLS handshake to
        // learn the peer's authenticated `EndpointId` (iroh authenticates the
        // peer's public key during the handshake — `remote_id()` is verified,
        // not self-asserted). If the peer is not on the allowlist we close the
        // connection immediately and serve no streams.
        tokio::spawn(async move {
            let conn = match incoming.await {
                Ok(c) => c,
                Err(_) => return,
            };
            let remote = conn.remote_id();
            // Dispatch strictly on the negotiated ALPN: pairing is the only
            // protocol open to non-allowlisted peers, sync the only other one.
            let alpn = conn.alpn();
            if alpn == PAIRING_ALPN {
                inner.serve_pairing(conn).await;
                return;
            }
            if alpn != SYNC_ALPN {
                conn.close(iroh::endpoint::VarInt::from_u32(1), b"unsupported alpn");
                return;
            }
            if !inner.peers.is_allowed(&remote) {
                conn.close(iroh::endpoint::VarInt::from_u32(1), b"not allowlisted");
                return;
            }
            inner.peers.touch_last_seen(&remote);
            // AUDIT (2026-08-28, kimi): the allowlist MUST be re-checked per
            // stream, not only once per connection; otherwise a peer revoked
            // while its QUIC connection is still open keeps syncing until that
            // connection happens to drop. Streams are served one at a time, so
            // a peer cannot fan out unbounded concurrent sessions.
            while let Ok((send, recv)) = conn.accept_bi().await {
                if !inner.peers.is_allowed(&remote) {
                    conn.close(iroh::endpoint::VarInt::from_u32(1), b"revoked");
                    break;
                }
                match inner.session(remote, Role::Acceptor).run(recv, send).await {
                    Ok(_) => inner.peers.touch_last_seen(&remote),
                    Err(e) => {
                        tracing::warn!(
                            "sync-p2p: inbound session with {} failed: {e}",
                            remote.fmt_short()
                        )
                    }
                }
            }
        });
    }
}

// ---------------------------------------------------------------------------
// peer direct-address registry (Loopback mode only)
// ---------------------------------------------------------------------------

// A process-local registry mapping a peer node id → the iroh socket addresses
// it is reachable on. Populated directly by tests (all agents live in the same
// process). Only consulted in `AgentTransport::Loopback` — `Discovered`
// resolves peers through iroh's relay/DNS address-lookup service instead (SYNC-8).

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

/// Register a peer's iroh direct addresses so a `Loopback`-mode agent can dial
/// it without discovery. (Same-machine proof / tests only; `Discovered` mode
/// uses relay/DNS instead — SYNC-8.)
pub async fn register_direct_addr(node_id: PublicKey, addrs: Vec<std::net::SocketAddr>) {
    let mut map = direct_addrs_map().await;
    map.as_mut().unwrap().insert(*node_id.as_bytes(), addrs);
}

/// AUDIT (doc §854, carried from the SYNC-3 audit as a SYNC-8 finding): this
/// used to reach for the registry with `try_lock` and fall back to an **empty
/// address list** on any contention, producing a spurious dial failure under
/// concurrent load. `dial_peer` is async, so `.await`ing the lock removes the
/// false failure mode entirely.
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

    /// REGRESSION (doc §854): `lookup_direct_addrs` used to reach for the
    /// registry with `try_lock` and silently fall back to an empty address
    /// list on any contention. This test holds the lock on a background task
    /// long enough to force the contention, and fails against the pre-fix
    /// `try_lock` code (which would observe the lock held and return `vec![]`).
    #[tokio::test]
    async fn lookup_direct_addrs_awaits_the_lock_instead_of_dropping_addrs_under_contention() {
        let node_id = Identity::for_test().id();
        let addr: std::net::SocketAddr = "127.0.0.1:9".parse().unwrap();
        register_direct_addr(node_id, vec![addr]).await;

        let held = tokio::spawn(async {
            let g = DIRECT_ADDRS.lock().await;
            tokio::time::sleep(Duration::from_millis(150)).await;
            drop(g);
        });
        // Give the background task a chance to grab the lock first.
        tokio::time::sleep(Duration::from_millis(30)).await;

        let found = lookup_direct_addrs(&node_id).await;
        held.await.unwrap();

        assert_eq!(
            found,
            vec![addr],
            "lookup must wait for the lock and return the registered address, not fall back to empty"
        );
    }
}

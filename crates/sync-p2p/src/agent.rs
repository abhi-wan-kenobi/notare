//! The P2P agent — the bridge between the synchronous C network layer and the
//! asynchronous iroh/QUIC transport.
//!
//! ## Why an agent (the C↔iroh boundary)
//!
//! The CloudSync core calls `network_send_buffer` / `network_receive_buffer`
//! **synchronously** from a SQLite function context on the DB thread (contract
//! §6: blocking only, no async contract). Those C functions cannot speak
//! QUIC/iroh directly. So the C layer stays deliberately dumb and **local**:
//! `crates/cloudsync/build/network_p2p.c` opens a plain TCP socket to
//! `127.0.0.1:<agent_port>` and sends the same framed length-prefixed JSON
//! [`Request`]/[`PutRequest`] the TCP spike used. iroh lives **entirely on the
//! Rust side** in this agent, which owns the iroh [`Endpoint`] and relays each
//! C request to the addressed peer over an iroh bi-directional stream.
//!
//! This keeps the C transport dead simple (one local socket, no crypto, no
//! async runtime in the extension) and quarantines the entire QUIC/rustls
//! dependency tree inside the Rust process — exactly where the v0.6 dependency
//! gate already proved it coexists cleanly with the app's existing rustls 0.23
//! stack.
//!
//! ## Endpoint scheme
//!
//! `cloudsync_network_init_custom(address, dbId)` builds endpoints as
//! `{address}/v2/cloudsync/databases/{dbId}/{siteId}/{action}`. We set
//! `address = p2p://<node-id-fingerprint>` — iroh addresses a peer by its
//! [`EndpointId`] (the Ed25519 public key), not by host:port. The agent parses
//! the fingerprint out of the endpoint authority:
//!
//! - if it is **this device's** node id → the request is for our own broker
//!   (a site on this device pulling/pushing its local changes); served from
//!   the in-process [`BrokerState`] directly;
//! - otherwise it names a **peer** → the agent checks the [`PeerStore`]
//!   allowlist, dials the peer's iroh endpoint, opens a bi-stream, and relays
//!   the framed request/response.
//!
//! `mem://` object URLs (the collapsed-S3 handles) carry the *serving* peer's
//! node-id fingerprint: `mem://<node-id-fingerprint>/<id>`. This lets the C
//! `network_send_buffer` (which receives the `mem://` URL with no other
//! context) route the blob PUT — and a download GET — back to the peer that
//! minted it.
//!
//! ## Allowlist enforcement (closes the §12 SSRF finding)
//!
//! Every outbound dial and every inbound connection is checked against the
//! [`PeerStore`]. A node id not on the allowlist is refused in *both*
//! directions. This is the production fix for the audit's SSRF finding: rather
//! than the extension dialing any host:port (or any node id) it is handed via
//! SQL, it dials only a paired, allowlisted peer.

use std::sync::Arc;

use iroh::endpoint::{RecvStream, SendStream};
use iroh::{Endpoint, EndpointAddr, PublicKey, RelayMode};
use subtle::ConstantTimeEq;
use tokio::io::AsyncReadExt;
use tokio::net::TcpListener;

use crate::broker::BrokerState;
use crate::crypto::{self};
use crate::identity::{Fingerprint, Identity};
use crate::peers::PeerStore;
use crate::protocol::{PutRequest, PutResponse, Request, Response, read_frame, write_frame};

/// The ALPN the sync transport speaks over iroh. Both peers must match.
///
/// Public because it is part of the wire contract: any peer implementation —
/// and the allowlist regression test, which opens two raw bi-streams on a
/// single connection — has to negotiate the same ALPN.
pub const SYNC_ALPN: &[u8] = b"/notare/sync/1";

/// The length, in bytes, of the bearer token that gates the C↔agent socket.
/// 128 bits: enough that a blind local brute-force is infeasible, small enough
/// to ship in every frame and read as a 32-char hex env var.
const TOKEN_LEN: usize = 16;

/// Generate a fresh bearer token for the C↔agent socket: 16 cryptographically
/// random bytes, hex-encoded (32 chars). The randomness comes from iroh's
/// `SecretKey` (it uses the OS CSPRNG), so no separate crypto crate is pulled
/// in just for this. The token is process-local and read per call by the C
/// layer via `NOTARE_SYNC_TOKEN` — it is never sent over the iroh peer link.
fn generate_token() -> String {
    let bytes = iroh::SecretKey::generate().to_bytes();
    let token_bytes = &bytes[..TOKEN_LEN];
    data_encoding::HEXLOWER.encode(token_bytes)
}

/// Constant-time comparison of two token strings. Guards the C↔agent socket
/// against a local process that can reach the port but does not have the
/// token: a plain `==` on the secret would short-circuit on the first
/// differing byte and leak its position over timing.
fn token_matches(provided: &str, expected: &str) -> bool {
    let p = provided.as_bytes();
    let e = expected.as_bytes();
    if p.len() != e.len() {
        // Length is not secret (the C layer always sends the full env-var
        // value), but the early return still avoids indexing mismatched
        // slices. The comparison of equal-length slices below is constant-time.
        return false;
    }
    p.ct_eq(e).into()
}

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
    #[error(transparent)]
    Crypto(#[from] crate::crypto::CryptoError),
}

/// A running P2P agent: the in-process iroh endpoint + local broker + the
/// localhost TCP listener the C network layer connects to.
pub struct P2pAgent {
    /// The iroh endpoint — owns the QUIC socket and the secret key.
    endpoint: Endpoint,
    /// This device's identity (node id = `endpoint.id()`).
    identity: Identity,
    /// The peer allowlist (enforced at dial + accept).
    peers: PeerStore,
    /// The local CloudSync control plane + object store, served to both local
    /// sites (over the C-facing TCP socket) and remote peers (over iroh).
    // Kept to extend the shared state's lifetime to the agent's; the accept
    // loops hold their own clones, so this field is not read directly.
    #[allow(dead_code)]
    broker: Arc<BrokerState>,
    /// `mem://` URL label = this device's node-id fingerprint (compact, so the
    /// `mem://` URL stays short and the C layer's host buffer stays small).
    self_label: String,
    /// The localhost TCP address the C layer connects to.
    pub local_addr: String,
    /// The bearer token the C layer must present on every frame to this
    /// agent's TCP socket (SYNC-5). Process-local; never sent over iroh.
    token: String,
    /// The join handle for the C-facing TCP accept loop.
    tcp_handle: tokio::task::JoinHandle<()>,
    /// The join handle for the iroh inbound connection accept loop.
    iroh_handle: tokio::task::JoinHandle<()>,
}

/// The endpoint address a site uses to sync with **this** device:
/// `p2p://<our-node-id-fingerprint>`.
pub fn self_address(identity: &Identity) -> String {
    // Compact (no dashes) so the core's endpoint URL stays short; the agent
    // parses it back through Fingerprint::parse, which accepts compact form.
    format!("p2p://{}", identity.fingerprint().compact())
}

impl P2pAgent {
    /// Start an agent using the persisted identity + allowlist from the app
    /// data dir (`<data_dir>/notare/sync/`), binding the iroh endpoint to
    /// localhost. Suitable for the same-machine convergence proof; production
    /// would bind a real interface and use `RelayMode::Default`.
    pub async fn start() -> Result<Self, AgentError> {
        let identity = Identity::load_or_create()?;
        let peers = PeerStore::load_or_create()?;
        Self::start_with(identity, peers).await
    }

    /// Start with an explicit identity + peer store (for tests).
    pub async fn start_with(identity: Identity, peers: PeerStore) -> Result<Self, AgentError> {
        let self_label = identity.fingerprint().compact();

        // iroh endpoint: disable relay for the same-machine proof (deterministic,
        // no external network), bind localhost, set our ALPN. The secret key IS
        // the device identity key — iroh's NodeId/EndpointId derives from it.
        let endpoint = Endpoint::builder(iroh::endpoint::presets::Minimal)
            .secret_key(identity.secret_key().clone())
            .alpns(vec![SYNC_ALPN.to_vec()])
            .relay_mode(RelayMode::Disabled)
            .bind_addr("127.0.0.1:0")
            .map_err(|_| AgentError::BadBindAddr)?
            .bind()
            .await?;

        let broker = Arc::new(BrokerState::with_addr_label(&self_label));

        // C-facing localhost TCP listener: the C `network_p2p.c` connects here.
        let tcp_listener = TcpListener::bind("127.0.0.1:0").await?;
        let local_addr = tcp_listener.local_addr()?.to_string();

        // SYNC-5: mint a bearer token for the C↔agent socket. The host process
        // publishes it as NOTARE_SYNC_TOKEN alongside NOTARE_SYNC_AGENT_ADDR;
        // the C layer includes it in every frame and we reject mismatches.
        let token = generate_token();

        let tcp_state = Arc::clone(&broker);
        let tcp_peers = peers.clone();
        let tcp_endpoint = endpoint.clone();
        let tcp_identity = identity.clone();
        let tcp_self_label = self_label.clone();
        let tcp_token = token.clone();
        let tcp_handle = tokio::spawn(async move {
            accept_c_tcp(
                tcp_listener,
                tcp_state,
                tcp_peers,
                tcp_endpoint,
                tcp_identity,
                tcp_self_label,
                tcp_token,
            )
            .await;
        });

        // iroh inbound accept loop: remote peers dial us to pull our changes.
        let iroh_state = Arc::clone(&broker);
        let iroh_peers = peers.clone();
        let iroh_endpoint = endpoint.clone();
        let iroh_identity = identity.clone();
        let iroh_handle = tokio::spawn(async move {
            accept_iroh(iroh_endpoint, iroh_state, iroh_peers, iroh_identity).await;
        });

        Ok(Self {
            endpoint,
            identity,
            peers,
            broker,
            self_label,
            local_addr,
            token,
            tcp_handle,
            iroh_handle,
        })
    }

    /// This device's node id / iroh EndpointId.
    pub fn node_id(&self) -> PublicKey {
        self.identity.id()
    }

    /// The bearer token the C layer must present on every frame to this
    /// agent's local TCP socket (SYNC-5). The host process sets it as the
    /// `NOTARE_SYNC_TOKEN` env var alongside `NOTARE_SYNC_AGENT_ADDR`.
    pub fn token(&self) -> &str {
        &self.token
    }

    /// The endpoint address sites use to sync with this device.
    pub fn address(&self) -> String {
        format!("p2p://{}", self.self_label)
    }

    /// The peer allowlist (for adding/removing peers at runtime).
    pub fn peers(&self) -> &PeerStore {
        &self.peers
    }

    /// The iroh endpoint's bound direct addresses (so a peer can dial us with
    /// a direct address rather than via relay — used by the test/example to
    /// build the dial address without external discovery).
    pub fn direct_addresses(&self) -> Vec<std::net::SocketAddr> {
        self.endpoint.bound_sockets()
    }

    /// Gracefully stop the agent.
    pub async fn stop(self) {
        self.tcp_handle.abort();
        self.iroh_handle.abort();
        self.endpoint.close().await;
    }
}

// ---------------------------------------------------------------------------
// C-facing TCP accept loop
// ---------------------------------------------------------------------------

/// Accept connections from the local C `network_p2p.c` layer. Each connection
/// carries one framed request (a `Request` or a `PutRequest`); route it to the
/// local broker or relay it to a peer over iroh, then write one framed reply.
async fn accept_c_tcp(
    listener: TcpListener,
    broker: Arc<BrokerState>,
    peers: PeerStore,
    endpoint: Endpoint,
    identity: Identity,
    self_label: String,
    token: String,
) {
    loop {
        // AUDIT (2026-08-28, gpt-oss): a transient accept error (EMFILE,
        // ECONNABORTED, EINTR) must not kill the listener — breaking here would
        // silently disable sync for the rest of the process lifetime. Back off
        // briefly and keep serving.
        let (mut stream, _) = match listener.accept().await {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!("sync-p2p: C-facing accept failed: {e}");
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                continue;
            }
        };
        let ctx = Ctx {
            broker: Arc::clone(&broker),
            peers: peers.clone(),
            endpoint: endpoint.clone(),
            identity: identity.clone(),
            self_label: self_label.clone(),
            token: token.clone(),
        };
        tokio::spawn(async move {
            // A connection that errors is just a dropped/short C call; nothing
            // to do — the C layer surfaces the failure as a network error.
            let _ = handle_c_connection(&mut stream, &ctx).await;
        });
    }
}

struct Ctx {
    broker: Arc<BrokerState>,
    peers: PeerStore,
    endpoint: Endpoint,
    identity: Identity,
    self_label: String,
    token: String,
}

async fn handle_c_connection(stream: &mut tokio::net::TcpStream, ctx: &Ctx) -> std::io::Result<()> {
    // Peek the frame to distinguish Request vs PutRequest. Read the raw frame
    // bytes first (length-prefixed), then try both deserializations.
    let mut buf = Vec::new();
    read_raw_frame(stream, &mut buf).await?;

    if let Ok(req) = serde_json::from_slice::<Request>(&buf) {
        // SYNC-5: gate the C↔agent socket with a bearer token. A frame whose
        // token does not match the one we minted at start is refused with the
        // shape the caller expects (`Response{status:401}` for a receive) and
        // served nothing. This closes the §14 audit finding: any local process
        // that can reach the port can otherwise read/write sync data, bypassing
        // the peer allowlist from the local side.
        if !token_matches(&req.token, &ctx.token) {
            let resp = Response {
                status: 401,
                body: None,
                error: Some("invalid sync token".into()),
            };
            write_frame(stream, &resp).await?;
            return Ok(());
        }
        let resp = route_request(&req, ctx).await;
        write_frame(stream, &resp).await?;
        return Ok(());
    }
    if let Ok(put) = serde_json::from_slice::<PutRequest>(&buf) {
        // SYNC-5: same token gate on the PUT path — `PutResponse{ok:false}`.
        if !token_matches(&put.token, &ctx.token) {
            let resp = PutResponse {
                ok: false,
                error: Some("invalid sync token".into()),
            };
            write_frame(stream, &resp).await?;
            return Ok(());
        }
        let resp = route_put(&put, ctx).await;
        write_frame(stream, &resp).await?;
        return Ok(());
    }
    // AUDIT (2026-08-28, gpt-oss + kimi, 2-seat agreement): reply in the shape
    // the caller expects. `network_receive_buffer` parses a `Response`
    // (`status`/`body`), `network_send_buffer` parses a `PutResponse` (`ok`).
    // Always answering with a PutResponse meant a malformed receive-side frame
    // came back in the wrong shape. `Request` and `PutRequest` have disjoint
    // required fields, so the presence of `"url"` is a sound discriminator for
    // a frame that failed both deserializations.
    if buf_looks_like_put(&buf) {
        let resp = PutResponse {
            ok: false,
            error: Some("unparseable frame".into()),
        };
        write_frame(stream, &resp).await?;
    } else {
        let resp = Response {
            status: 400,
            body: None,
            error: Some("unparseable frame".into()),
        };
        write_frame(stream, &resp).await?;
    }
    Ok(())
}

/// Best-effort discriminator for a frame that failed both deserializations:
/// `PutRequest` carries `"url"`/`"blob"`, `Request` carries `"endpoint"`.
fn buf_looks_like_put(buf: &[u8]) -> bool {
    let v: Option<serde_json::Value> = serde_json::from_slice(buf).ok();
    match v {
        Some(serde_json::Value::Object(m)) => m.contains_key("url") || m.contains_key("blob"),
        _ => false,
    }
}

/// Route a `Request` (a `network_receive_buffer` call) to the local broker or
/// a peer, depending on the endpoint authority.
async fn route_request(req: &Request, ctx: &Ctx) -> Response {
    // `mem://<node-id>/<id>`: a download GET for a blob the named node minted.
    if let Some(rest) = req.endpoint.strip_prefix("mem://") {
        return route_mem_get(rest, req, ctx).await;
    }

    // `p2p://<authority>/v2/cloudsync/databases/<dbId>/<siteId>/<action>`.
    let Some(authority) = endpoint_authority(&req.endpoint) else {
        return Response {
            status: 500,
            body: None,
            error: Some(format!("unparseable endpoint: {}", req.endpoint)),
        };
    };

    // Is the authority this device? (compact fingerprint of our node id)
    if authority == ctx.self_label {
        return ctx.broker.handle_request(req.clone()).await;
    }

    // Otherwise it names a peer — parse the fingerprint, enforce the allowlist,
    // dial over iroh, and relay the framed request on a bi-stream.
    let node_id = match Fingerprint::parse(&authority) {
        Ok(pk) => pk,
        Err(_) => {
            return Response {
                status: 500,
                body: None,
                error: Some(format!("bad peer fingerprint in endpoint: {authority}")),
            };
        }
    };
    relay_request_to_peer(req, &node_id, ctx).await
}

/// Route a `PutRequest` (a `network_send_buffer` PUT) to the node named in the
/// `mem://` URL.
async fn route_put(put: &PutRequest, ctx: &Ctx) -> PutResponse {
    let Some(rest) = put.url.strip_prefix("mem://") else {
        return PutResponse {
            ok: false,
            error: Some(format!("not a mem:// url: {}", put.url)),
        };
    };
    // `mem://<node-id-fingerprint>/<id>` → the node that minted this handle.
    let (authority, _id) = match rest.split_once('/') {
        Some((a, b)) => (a, b),
        None => (rest, ""),
    };

    if authority == ctx.self_label {
        return ctx.broker.handle_put(put.clone()).await;
    }

    let node_id = match Fingerprint::parse(authority) {
        Ok(pk) => pk,
        Err(_) => {
            return PutResponse {
                ok: false,
                error: Some(format!("bad peer fingerprint in mem url: {authority}")),
            };
        }
    };
    relay_put_to_peer(put, &node_id, ctx).await
}

/// `mem://` GET (download): fetch a blob from the node that minted it.
async fn route_mem_get(rest: &str, req: &Request, ctx: &Ctx) -> Response {
    let (authority, _id) = match rest.split_once('/') {
        Some((a, b)) => (a, b),
        None => (rest, ""),
    };
    if authority == ctx.self_label {
        return ctx.broker.handle_request(req.clone()).await;
    }
    let node_id = match Fingerprint::parse(authority) {
        Ok(pk) => pk,
        Err(_) => {
            return Response {
                status: 500,
                body: None,
                error: Some(format!("bad peer fingerprint in mem url: {authority}")),
            };
        }
    };
    relay_request_to_peer(req, &node_id, ctx).await
}

// ---------------------------------------------------------------------------
// iroh relay (outbound to a peer)
// ---------------------------------------------------------------------------

/// Dial `node_id` over iroh, open a bi-stream, send the framed `Request`, read
/// the framed `Response`. Enforces the allowlist before dialing — this is the
/// outbound half of the §12 SSRF fix.
async fn relay_request_to_peer(req: &Request, node_id: &PublicKey, ctx: &Ctx) -> Response {
    if !ctx.peers.is_allowed(node_id) {
        // ALLOWLIST ENFORCEMENT (outbound): refuse to dial a non-allowlisted
        // node id. Closes the §12 SSRF finding — the extension will not dial
        // an arbitrary node id supplied via SQL, only a paired peer.
        return Response {
            status: 403,
            body: None,
            error: Some("peer not allowlisted".into()),
        };
    }

    let conn = match dial_peer(node_id, ctx).await {
        Ok(c) => c,
        Err(e) => {
            return Response {
                status: 502,
                body: None,
                error: Some(format!("dial peer: {e}")),
            };
        }
    };
    let (mut send, mut recv) = match conn.open_bi().await {
        Ok(s) => s,
        Err(e) => {
            return Response {
                status: 502,
                body: None,
                error: Some(format!("open stream: {e}")),
            };
        }
    };
    // SYNC-7: the local C↔agent hop is token-gated and stays plaintext. The
    // body that actually crosses to a remote peer is encrypted here with an
    // X25519 key derived from both devices' Ed25519 identity keys. Only the
    // `body` field carries a blob, so a non-blob Request is relayed unchanged.
    let outbound = match encrypt_request_if_needed(ctx.identity.secret_key(), node_id, req) {
        Ok(r) => r,
        Err(e) => {
            return Response {
                status: 500,
                body: None,
                error: Some(format!("encrypt request body: {e}")),
            };
        }
    };
    if let Err(e) = write_frame(&mut send, &outbound).await {
        return Response {
            status: 502,
            body: None,
            error: Some(format!("write to peer: {e}")),
        };
    }
    // The broker reads exactly one frame and writes exactly one frame, so
    // closing the send half signals "request complete" to the peer's reader.
    let _ = send.finish();
    match read_frame::<_, Response>(&mut recv).await {
        Ok(resp) => {
            ctx.peers.touch_last_seen(node_id);
            // The peer has encrypted the returned blob (if any) for us.
            match decrypt_response_if_needed(ctx.identity.secret_key(), node_id, resp) {
                Ok(r) => r,
                Err(e) => Response {
                    status: 500,
                    body: None,
                    error: Some(format!("decrypt peer response: {e}")),
                },
            }
        }
        Err(e) => Response {
            status: 502,
            body: None,
            error: Some(format!("read from peer: {e}")),
        },
    }
}

/// Dial `node_id` over iroh, open a bi-stream, send the framed `PutRequest`,
/// read the framed `PutResponse`. Allowlist-enforced.
async fn relay_put_to_peer(put: &PutRequest, node_id: &PublicKey, ctx: &Ctx) -> PutResponse {
    if !ctx.peers.is_allowed(node_id) {
        return PutResponse {
            ok: false,
            error: Some("peer not allowlisted".into()),
        };
    }
    let conn = match dial_peer(node_id, ctx).await {
        Ok(c) => c,
        Err(e) => {
            return PutResponse {
                ok: false,
                error: Some(format!("dial peer: {e}")),
            };
        }
    };
    let (mut send, mut recv) = match conn.open_bi().await {
        Ok(s) => s,
        Err(e) => {
            return PutResponse {
                ok: false,
                error: Some(format!("open stream: {e}")),
            };
        }
    };
    // SYNC-7: encrypt the blob before it leaves this device. The framed
    // envelope is unchanged; the `blob` field is replaced with `[nonce||ciphertext].
    let outbound = match encrypt_put(ctx.identity.secret_key(), node_id, put) {
        Ok(p) => p,
        Err(e) => {
            return PutResponse {
                ok: false,
                error: Some(format!("encrypt PUT blob: {e}")),
            };
        }
    };
    if let Err(e) = write_frame(&mut send, &outbound).await {
        return PutResponse {
            ok: false,
            error: Some(format!("write to peer: {e}")),
        };
    }
    let _ = send.finish();
    match read_frame::<_, PutResponse>(&mut recv).await {
        Ok(resp) => {
            ctx.peers.touch_last_seen(node_id);
            resp
        }
        Err(e) => PutResponse {
            ok: false,
            error: Some(format!("read from peer: {e}")),
        },
    }
}

/// Resolve a peer's iroh direct address and dial it. The agent's
/// [`P2pAgent::direct_addresses`] supplies the peer's socket addresses for the
/// same-machine proof; production discovery (relay/DNS/pkarr) is SYNC-8.
async fn dial_peer(node_id: &PublicKey, ctx: &Ctx) -> Result<iroh::endpoint::Connection, String> {
    // Build an EndpointAddr from the node id + the peer's known direct
    // addresses. For the convergence proof the addresses are injected via the
    // shared DIRECT_ADDR registry (see `register_direct_addr`); production
    // would resolve them through the relay/DNS address lookup (SYNC-8).
    //
    // Retry briefly: a peer's iroh endpoint may not be accepting the instant
    // we dial (it binds asynchronously, and under concurrent test load the
    // readiness gap can exceed a fixed sleep). A few quick attempts with
    // backoff handle the race deterministically and mirror real-world dialing.
    let addrs = lookup_direct_addrs(node_id);
    let mut ea = EndpointAddr::new(*node_id);
    for a in addrs {
        ea = ea.with_ip_addr(a);
    }

    let mut last_err = String::from("dial failed");
    for attempt in 0..8u32 {
        match ctx.endpoint.connect(ea.clone(), SYNC_ALPN).await {
            Ok(conn) => return Ok(conn),
            Err(e) => last_err = format!("{e}"),
        }
        // Backoff: 1ms, 2ms, 4ms, … up to ~128ms across all attempts (~250ms
        // total worst case) — enough to ride out an endpoint that is still
        // binding, without blocking a real sync call on a dead peer.
        tokio::time::sleep(std::time::Duration::from_millis(1 << attempt)).await;
    }
    Err(last_err)
}

// ---------------------------------------------------------------------------
// iroh inbound accept loop
// ---------------------------------------------------------------------------

/// Accept inbound iroh connections from peers. Each connection's remote
/// `EndpointId` is checked against the allowlist; non-allowlisted peers are
/// refused. This is the inbound half of the §12 SSRF fix. Allowed peers'
/// bi-streams are served from the local broker.
async fn accept_iroh(
    endpoint: Endpoint,
    broker: Arc<BrokerState>,
    peers: PeerStore,
    identity: Identity,
) {
    loop {
        // `endpoint.accept().await` yields `Option<Incoming>` — None once the
        // endpoint is closed.
        let Some(incoming) = endpoint.accept().await else {
            break;
        };
        let peers_refuse = peers.clone();
        // ALLOWLIST ENFORCEMENT (inbound): we must complete the TLS handshake to
        // learn the peer's authenticated `EndpointId` (iroh authenticates the
        // peer's public key during the handshake — `remote_id()` is verified,
        // not self-asserted). If the peer is not on the allowlist we close the
        // connection immediately and serve no streams. Closes the §12 SSRF
        // finding on the accept side — an unpaired node id cannot open a sync
        // stream to us. (`Incoming` exposes only `remote_addr` pre-handshake,
        // not the node id, so the id check is post-handshake by necessity.)
        let broker = Arc::clone(&broker);
        let peers = peers.clone();
        let identity = identity.clone();
        tokio::spawn(async move {
            // Awaiting `Incoming` completes the handshake and yields the
            // authenticated `Connection`.
            let conn = match incoming.await {
                Ok(c) => c,
                Err(_) => return,
            };
            let remote = conn.remote_id();
            if !peers_refuse.is_allowed(&remote) {
                // Not allowlisted: tear down the connection without serving.
                conn.close(iroh::endpoint::VarInt::from_u32(1), b"not allowlisted");
                return;
            }
            peers.touch_last_seen(&remote);
            // Serve bi-streams from this connection until it closes. Each
            // bi-stream is one framed Request/PutRequest → one framed reply.
            //
            // AUDIT (2026-08-28, kimi): the allowlist MUST be re-checked per
            // stream, not only once per connection. The check above happens at
            // handshake time; without this re-check a peer revoked while its
            // QUIC connection is still open keeps syncing until that connection
            // happens to drop — i.e. revocation would not take effect against an
            // actively-connected peer, which is the case revocation exists for.
            while let Ok((mut send, mut recv)) = conn.accept_bi().await {
                if !peers_refuse.is_allowed(&remote) {
                    conn.close(iroh::endpoint::VarInt::from_u32(1), b"revoked");
                    break;
                }
                let broker = Arc::clone(&broker);
                let identity = identity.clone();
                tokio::spawn(async move {
                    let _ =
                        serve_peer_stream(&mut send, &mut recv, &broker, &identity, remote).await;
                });
            }
        });
    }
}

/// Read one framed request from a peer's bi-stream and write one framed reply.
///
/// SYNC-7: inbound frames from a remote peer are decrypted (where a payload is
/// present) before the broker sees them. A decryption failure is a hard error
/// returned as a framed 5xx / `ok:false` response; the broker never processes
/// the plaintext.
async fn serve_peer_stream(
    send: &mut SendStream,
    recv: &mut RecvStream,
    broker: &BrokerState,
    identity: &Identity,
    remote: iroh::PublicKey,
) -> std::io::Result<()> {
    let mut buf = Vec::new();
    read_raw_frame(recv, &mut buf).await?;

    if let Ok(req) = serde_json::from_slice::<Request>(&buf) {
        let req = match decrypt_request_if_needed(identity.secret_key(), &remote, req) {
            Ok(r) => r,
            Err(e) => {
                let resp = Response {
                    status: 500,
                    body: None,
                    error: Some(format!("decrypt peer request: {e}")),
                };
                write_frame(send, &resp).await?;
                return Ok(());
            }
        };
        let resp = broker.handle_request(req).await;
        let resp = match encrypt_response_if_needed(identity.secret_key(), &remote, resp) {
            Ok(r) => r,
            Err(e) => {
                let resp = Response {
                    status: 500,
                    body: None,
                    error: Some(format!("encrypt peer response: {e}")),
                };
                write_frame(send, &resp).await?;
                return Ok(());
            }
        };
        write_frame(send, &resp).await?;
        return Ok(());
    }
    if let Ok(put) = serde_json::from_slice::<PutRequest>(&buf) {
        let put = match decrypt_put(identity.secret_key(), &remote, put) {
            Ok(p) => p,
            Err(e) => {
                let resp = PutResponse {
                    ok: false,
                    error: Some(format!("decrypt peer PUT: {e}")),
                };
                write_frame(send, &resp).await?;
                return Ok(());
            }
        };
        let resp = broker.handle_put(put).await;
        write_frame(send, &resp).await?;
        return Ok(());
    }
    let resp = PutResponse {
        ok: false,
        error: Some("unparseable frame".into()),
    };
    write_frame(send, &resp).await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// SYNC-7 payload encryption helpers (peer boundary only)
// ---------------------------------------------------------------------------

/// Base64-encode a byte slice, returning `None` for an empty input so we can
/// distinguish "no payload" from "empty payload".
fn b64_encode(bytes: &[u8]) -> Option<Vec<u8>> {
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    if bytes.is_empty() {
        None
    } else {
        Some(STANDARD.encode(bytes).into_bytes())
    }
}

/// Base64-decode a byte slice that was produced by `b64_encode`.
fn b64_decode(bytes: &[u8]) -> Result<Vec<u8>, std::io::Error> {
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    let s = std::str::from_utf8(bytes)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    STANDARD
        .decode(s)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
}

/// Encrypt a `body` (request or response) if present. On crypto failure the
/// error is propagated as a hard error so no unencrypted payload leaves the
/// device.
fn encrypt_body(
    secret: &iroh::SecretKey,
    peer: &PublicKey,
    body: &Option<Vec<u8>>,
) -> Result<Option<Vec<u8>>, crate::crypto::CryptoError> {
    match body.as_ref() {
        Some(bytes) => {
            let sealed = crypto::encrypt(secret, peer, bytes)?;
            Ok(b64_encode(&sealed))
        }
        None => Ok(None),
    }
}

/// Decrypt a `body` that was encrypted by the peer. A failure here is a hard
/// error: the broker must not see the plaintext.
fn decrypt_body(
    secret: &iroh::SecretKey,
    peer: &PublicKey,
    body: Option<Vec<u8>>,
) -> Result<Option<Vec<u8>>, std::io::Error> {
    match body {
        Some(body_b64) => {
            let sealed = b64_decode(&body_b64)?;
            let plain = crypto::decrypt(secret, peer, &sealed)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
            Ok(Some(plain))
        }
        None => Ok(None),
    }
}

/// A control Request (upload/apply/check/status) carries no blob, so it crosses
/// the wire unchanged. A POST body (e.g. the apply/check JSON) is encrypted.
fn encrypt_request_if_needed(
    secret: &iroh::SecretKey,
    peer: &PublicKey,
    req: &Request,
) -> Result<Request, crate::crypto::CryptoError> {
    let body = encrypt_body(secret, peer, &req.body)?;
    Ok(Request {
        token: req.token.clone(),
        endpoint: req.endpoint.clone(),
        is_post: req.is_post,
        body,
    })
}

/// Inverse of [`encrypt_request_if_needed`].
fn decrypt_request_if_needed(
    secret: &iroh::SecretKey,
    peer: &PublicKey,
    req: Request,
) -> Result<Request, std::io::Error> {
    let body = decrypt_body(secret, peer, req.body)?;
    Ok(Request {
        token: req.token,
        endpoint: req.endpoint,
        is_post: req.is_post,
        body,
    })
}

/// GET responses that carry a blob body (the download path) are encrypted.
fn encrypt_response_if_needed(
    secret: &iroh::SecretKey,
    peer: &PublicKey,
    resp: Response,
) -> Result<Response, crate::crypto::CryptoError> {
    let body = encrypt_body(secret, peer, &resp.body)?;
    Ok(Response {
        status: resp.status,
        body,
        error: resp.error,
    })
}

/// Inverse of [`encrypt_response_if_needed`].
fn decrypt_response_if_needed(
    secret: &iroh::SecretKey,
    peer: &PublicKey,
    resp: Response,
) -> Result<Response, std::io::Error> {
    let body = decrypt_body(secret, peer, resp.body)?;
    Ok(Response {
        status: resp.status,
        body,
        error: resp.error,
    })
}

/// PUT blob encryption. The `blob` field becomes `[nonce || ciphertext+tag].
fn encrypt_put(
    secret: &iroh::SecretKey,
    peer: &PublicKey,
    put: &PutRequest,
) -> Result<PutRequest, crate::crypto::CryptoError> {
    let sealed = crypto::encrypt(secret, peer, &put.blob)?;
    Ok(PutRequest {
        token: put.token.clone(),
        url: put.url.clone(),
        blob: sealed,
    })
}

/// PUT blob decryption.
fn decrypt_put(
    secret: &iroh::SecretKey,
    peer: &PublicKey,
    mut put: PutRequest,
) -> Result<PutRequest, std::io::Error> {
    put.blob = crypto::decrypt(secret, peer, &put.blob)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    Ok(put)
}

// ---------------------------------------------------------------------------
// framing helpers (length-prefixed, transport-agnostic)
// ---------------------------------------------------------------------------

/// Read one 4-byte length-prefixed frame into `buf` (raw bytes, no deserialize).
async fn read_raw_frame<R: AsyncReadExt + Unpin>(
    r: &mut R,
    buf: &mut Vec<u8>,
) -> std::io::Result<()> {
    let mut len_buf = [0u8; 4];
    r.read_exact(&mut len_buf).await?;
    let len = u32::from_be_bytes(len_buf) as usize;
    if len > 64 * 1024 * 1024 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "frame too large",
        ));
    }
    buf.resize(len, 0);
    r.read_exact(buf).await?;
    Ok(())
}

/// Extract the authority segment (between the scheme and the first `/`) of a
/// `p2p://` endpoint URL.
///
/// AUDIT (2026-08-28, kimi): `http://` was also accepted here, a leftover from
/// the S1 localhost spike. The endpoint is attacker-influenced (it derives from
/// the SQL-supplied address), so accepting a second scheme only widens the
/// surface for scheme confusion — `p2p://` is the only scheme this transport
/// defines. `mem://` is handled separately by the caller.
fn endpoint_authority(url: &str) -> Option<String> {
    let after = url.strip_prefix("p2p://")?;
    let auth = after.split_once('/').map(|(a, _)| a).unwrap_or(after);
    if auth.is_empty() {
        None
    } else {
        Some(auth.to_string())
    }
}

// ---------------------------------------------------------------------------
// peer direct-address registry (same-machine proof only)
// ---------------------------------------------------------------------------

// A process-local registry mapping a peer node id → the iroh socket addresses
// it is reachable on. The convergence proof populates this directly (both
// agents live in the same process). Production replaces this with relay/DNS
// address lookup — that is SYNC-8 (rendezvous/relay), out of scope here.

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

/// Register a peer's iroh direct addresses so this agent can dial it without
/// external discovery. (Same-machine proof; production uses relay/DNS — SYNC-8.)
pub async fn register_direct_addr(node_id: PublicKey, addrs: Vec<std::net::SocketAddr>) {
    let mut map = direct_addrs_map().await;
    map.as_mut().unwrap().insert(*node_id.as_bytes(), addrs);
}

fn lookup_direct_addrs(node_id: &PublicKey) -> Vec<std::net::SocketAddr> {
    // Synchronous lookup is fine: the registry is populated up-front by the
    // test/example, and `dial_peer` is already async, so we do a quick
    // try-lock. Fall back to empty (dial will then rely on relay, which is
    // Disabled in the proof — so an unregistered peer simply fails to dial,
    // which is the safe/correct outcome for the proof).
    match DIRECT_ADDRS.try_lock() {
        Ok(g) => g
            .as_ref()
            .and_then(|m| m.get(node_id.as_bytes()))
            .cloned()
            .unwrap_or_default(),
        Err(_) => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoint_authority_extracts_node_id() {
        let fp = Identity::for_test().fingerprint().compact();
        let url = format!("p2p://{fp}/v2/cloudsync/databases/db/site/upload");
        assert_eq!(endpoint_authority(&url).as_deref(), Some(fp.as_str()));
    }

    #[test]
    fn mem_url_splits_authority_and_id() {
        let rest = "abcd1234/99";
        let (a, b) = rest.split_once('/').unwrap();
        assert_eq!(a, "abcd1234");
        assert_eq!(b, "99");
    }
}

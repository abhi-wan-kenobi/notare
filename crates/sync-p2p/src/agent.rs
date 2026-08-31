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
    /// Deterministic and fully offline — what every test and the three
    /// `examples/` use.
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
    /// enabled (`AgentTransport::Discovered`). This is the app entry point —
    /// `plugins/db/src/sync.rs` calls this, never `start_with`.
    pub async fn start() -> Result<Self, AgentError> {
        let identity = Identity::load_or_create()?;
        let peers = PeerStore::load_or_create()?;
        Self::start_with_transport(identity, peers, AgentTransport::Discovered).await
    }

    /// Start with an explicit identity + peer store, on `AgentTransport::Loopback`
    /// (today's same-machine, deterministic, offline behavior). Existing
    /// callers — every test and the three `examples/` — use this unchanged;
    /// production discovery is opted into via [`Self::start_with_transport`].
    pub async fn start_with(identity: Identity, peers: PeerStore) -> Result<Self, AgentError> {
        Self::start_with_transport(identity, peers, AgentTransport::Loopback).await
    }

    /// Start with an explicit identity, peer store, and transport config.
    pub async fn start_with_transport(
        identity: Identity,
        peers: PeerStore,
        transport: AgentTransport,
    ) -> Result<Self, AgentError> {
        let self_label = identity.fingerprint().compact();

        // The secret key IS the device identity key — iroh's NodeId/EndpointId
        // derives from it — in both modes.
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
                    .alpns(vec![SYNC_ALPN.to_vec()])
                    .relay_mode(RelayMode::Default)
                    .bind()
                    .await?
            }
        };

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
                transport,
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
    transport: AgentTransport,
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
            transport,
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
    /// Governs `dial_peer`: `Loopback` injects addresses from the process-local
    /// registry, `Discovered` dials by node id alone and lets iroh resolve it.
    transport: AgentTransport,
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

/// Resolve `node_id` to an [`EndpointAddr`] and dial it. In `Loopback` mode the
/// agent's own [`P2pAgent::direct_addresses`] (via the process-local
/// `DIRECT_ADDRS` registry) supplies the peer's socket addresses directly —
/// there is no relay and no address lookup to fall back on. In `Discovered`
/// mode no addresses are injected: `EndpointAddr::new(node_id)` names the
/// peer by node id alone, and iroh's relay/DNS address-lookup service (wired
/// up in [`P2pAgent::start_with_transport`]) resolves it (SYNC-8).
async fn dial_peer(node_id: &PublicKey, ctx: &Ctx) -> Result<iroh::endpoint::Connection, String> {
    let mut ea = EndpointAddr::new(*node_id);
    if ctx.transport == AgentTransport::Loopback {
        let addrs = lookup_direct_addrs(node_id).await;
        for a in addrs {
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
    // multi-minute hang, which is exactly the "no hang" requirement this dial
    // ladder exists to satisfy. Each attempt is therefore wrapped in its own
    // `tokio::time::timeout` so a non-responding peer fails on this ladder's
    // schedule, not iroh's.
    let mut last_err = String::from("dial failed");
    for attempt in 0..dial_attempts(ctx.transport) {
        match tokio::time::timeout(
            dial_attempt_timeout(ctx.transport),
            ctx.endpoint.connect(ea.clone(), SYNC_ALPN),
        )
        .await
        {
            Ok(Ok(conn)) => return Ok(conn),
            Ok(Err(e)) => last_err = format!("{e}"),
            Err(_) => last_err = "dial attempt timed out".to_string(),
        }
        tokio::time::sleep(dial_backoff(ctx.transport, attempt)).await;
    }
    Err(last_err)
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
/// `Loopback`: unchanged from the original same-machine proof — 1, 2, 4, …,
/// 128ms (~255ms total across 8 attempts). That ladder only ever needs to
/// ride out a same-process peer endpoint that is still finishing its own
/// `bind()` under concurrent test load; the peer is either up within a couple
/// hundred ms or it never will be, so a WAN-scale ladder here would just make
/// every offline-peer test slow for no benefit.
///
/// `Discovered`: retuned for a real relayed QUIC dial. The original ladder's
/// ~128ms ceiling assumed a peer already reachable on localhost; over a real
/// network a `connect()` may need to complete a DNS/pkarr address-lookup
/// round trip and/or a relay handshake before the QUIC handshake can even
/// start, which routinely costs hundreds of ms to low seconds even on a
/// healthy path. 250ms, 500ms, 1s, 2s, 4s, 8s (~15.75s total across 6
/// attempts) gives a WAN dial room to complete without turning a genuinely
/// offline peer into a multi-minute hang — bounded, not unbounded.
fn dial_backoff(transport: AgentTransport, attempt: u32) -> std::time::Duration {
    let base_ms: u64 = match transport {
        AgentTransport::Loopback => 1,
        AgentTransport::Discovered => 250,
    };
    std::time::Duration::from_millis(base_ms << attempt)
}

/// Cap on a single `connect()` attempt in `dial_peer`. See the `FINDING` note
/// at the call site: without this, a peer bound to a dead address (not merely
/// slow) leaves iroh waiting on its own internal handshake/idle timeout —
/// tens of seconds — with no per-attempt bound to fail fast on instead.
///
/// `Loopback`: 500ms is generous for a same-machine handshake (normally single
/// digit ms) while still bounding a dead-port dial. `Discovered`: 5s allows
/// room for a real DNS/pkarr address-lookup round trip plus a relay-assisted
/// QUIC handshake, which can legitimately take a couple of seconds on a
/// healthy WAN path.
fn dial_attempt_timeout(transport: AgentTransport) -> std::time::Duration {
    match transport {
        AgentTransport::Loopback => std::time::Duration::from_millis(500),
        AgentTransport::Discovered => std::time::Duration::from_secs(5),
    }
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
// peer direct-address registry (Loopback mode only)
// ---------------------------------------------------------------------------

// A process-local registry mapping a peer node id → the iroh socket addresses
// it is reachable on. Populated directly by tests/examples (all agents live in
// the same process). Only consulted in `AgentTransport::Loopback` — `Discovered`
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
/// used to be a synchronous `fn` that reached for the registry with
/// `DIRECT_ADDRS.try_lock()` and fell back to an **empty address list** on any
/// contention, producing a spurious dial failure under concurrent load rather
/// than an actual "peer unknown" outcome. `dial_peer` (the only caller) is
/// already async, so there was never a reason to avoid the wait — `.await`ing
/// the lock removes the false failure mode entirely. See
/// `lookup_direct_addrs_awaits_the_lock_instead_of_dropping_addrs_under_contention`
/// below for the regression test.
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

    /// REGRESSION (doc §854): `lookup_direct_addrs` used to reach for the
    /// registry with `try_lock` and silently fall back to an empty address
    /// list on any contention, producing a spurious dial failure. `dial_peer`
    /// is already async, so the fix `.await`s the lock instead. This test
    /// holds the lock on a background task long enough to force the
    /// contention, and fails against the pre-fix `try_lock` code (which would
    /// observe the lock held and return `vec![]`).
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
            "lookup must wait for the lock and return the registered address, not fall back to empty"
        );
    }

    /// Requirement: discovery changes *how* a peer is found, never *whether*
    /// it is allowed. `AgentTransport::Discovered` is the mode where a peer
    /// is genuinely reachable from the open internet the moment it is
    /// discoverable — this pins that the allowlist gate in
    /// `relay_request_to_peer` / `relay_put_to_peer` still runs, and still
    /// refuses, strictly before `dial_peer` is ever called. The endpoint
    /// itself only needs to exist (Loopback-shaped, built offline) because a
    /// non-allowlisted node id must never reach the point where the
    /// endpoint's real capabilities (relay, discovery) would matter.
    #[tokio::test]
    async fn discovered_mode_still_refuses_an_unpaired_but_reachable_peer() {
        let dir = tempfile::tempdir().unwrap();
        let broker = Arc::new(BrokerState::with_addr_label("self"));
        // Empty allowlist: the peer below is not paired.
        let peers = PeerStore::load_or_create_in(dir.path()).unwrap();
        let identity = Identity::for_test();

        let endpoint = Endpoint::builder(iroh::endpoint::presets::Minimal)
            .secret_key(identity.secret_key().clone())
            .alpns(vec![SYNC_ALPN.to_vec()])
            .relay_mode(RelayMode::Disabled)
            .bind_addr("127.0.0.1:0")
            .unwrap()
            .bind()
            .await
            .unwrap();

        let ctx = Ctx {
            broker,
            peers,
            endpoint,
            identity,
            self_label: "self".to_string(),
            token: "token".to_string(),
            transport: AgentTransport::Discovered,
        };

        let unpaired = Identity::for_test().id();
        let req = Request {
            token: "token".to_string(),
            endpoint: format!(
                "p2p://{}/v2/cloudsync/databases/db/site/upload",
                Fingerprint::from_pubkey(&unpaired).compact()
            ),
            is_post: false,
            body: None,
        };
        let resp = relay_request_to_peer(&req, &unpaired, &ctx).await;
        assert_eq!(
            resp.status, 403,
            "unpaired peer refused before dial (Discovered, Request)"
        );
        assert!(resp.error.unwrap().contains("not allowlisted"));

        let put = PutRequest {
            token: "token".to_string(),
            url: format!("mem://{}/id", Fingerprint::from_pubkey(&unpaired).compact()),
            blob: b"x".to_vec(),
        };
        let put_resp = relay_put_to_peer(&put, &unpaired, &ctx).await;
        assert!(
            !put_resp.ok,
            "unpaired peer refused before dial (Discovered, Put)"
        );
        assert!(put_resp.error.unwrap().contains("not allowlisted"));
    }
}

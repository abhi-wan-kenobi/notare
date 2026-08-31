//! Integration tests for the iroh P2P transport + allowlist enforcement.
//!
//! These drive two [`P2pAgent`]s against each other over real iroh/QUIC
//! (loopback, relay disabled) and assert:
//!   - an allowlisted peer can be dialed and served (the relay path works);
//!   - a non-allowlisted node id is refused on the **dial** side (outbound
//!     SSRF gate, §12) — the agent returns a 403 and never opens a stream;
//!   - a non-allowlisted node id is refused on the **accept** side (inbound
//!     SSRF gate, §12) — the connection is closed with no streams served.
//!   - the C↔agent socket requires the SYNC-5 bearer token (missing or wrong
//!     token → 401 / `ok:false`, both frame shapes) — a token-less local
//!     process can no longer bypass the peer allowlist.
//!
//! Same-machine only (both agents in one process); production NAT traversal
//! is iroh's job and out of scope for this PR.

use std::time::Duration;

use sync_p2p::protocol::{PutRequest, Request, Response};
use sync_p2p::{AgentTransport, Identity, P2pAgent, PeerStore, register_direct_addr};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

/// The ALPN-matching, framed-over-TCP client the C `network_p2p.c` layer is a
/// minimal version of: connect to the agent's local TCP port, write one
/// length-prefixed frame, read one length-prefixed frame back.
async fn agent_roundtrip(local_addr: &str, req: &Request) -> std::io::Result<Response> {
    let mut stream = TcpStream::connect(local_addr).await?;
    let json = serde_json::to_vec(req).unwrap();
    let len = (json.len() as u32).to_be_bytes();
    stream.write_all(&len).await?;
    stream.write_all(&json).await?;
    stream.flush().await?;

    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf).await?;
    let len = u32::from_be_bytes(len_buf) as usize;
    let mut buf = vec![0u8; len];
    stream.read_exact(&mut buf).await?;
    Ok(serde_json::from_slice(&buf).unwrap())
}

/// Bring up two agents in temp data dirs, each allowlisting the other, and
/// register their direct addresses so they can dial without relay/discovery.
async fn two_allowlisted_agents() -> (P2pAgent, P2pAgent) {
    let dir_a = tempfile::tempdir().unwrap();
    let dir_b = tempfile::tempdir().unwrap();

    let id_a = Identity::load_or_create_in(dir_a.path()).unwrap();
    let id_b = Identity::load_or_create_in(dir_b.path()).unwrap();

    let peers_a = PeerStore::load_or_create_in(dir_a.path()).unwrap();
    let peers_b = PeerStore::load_or_create_in(dir_b.path()).unwrap();

    // Each allowlists the other's node id.
    peers_a.add_peer(id_b.id(), "B").unwrap();
    peers_b.add_peer(id_a.id(), "A").unwrap();

    let agent_a = P2pAgent::start_with(id_a, peers_a).await.unwrap();
    let agent_b = P2pAgent::start_with(id_b, peers_b).await.unwrap();

    // Register direct addresses so dial_peer can reach each without relay
    // (RelayMode::Disabled in the proof).
    register_direct_addr(agent_a.node_id(), agent_a.direct_addresses()).await;
    register_direct_addr(agent_b.node_id(), agent_b.direct_addresses()).await;

    // Give the iroh endpoints a moment to bind and be ready.
    tokio::time::sleep(Duration::from_millis(100)).await;

    (agent_a, agent_b)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn allowlisted_peer_is_served_over_iroh() {
    let (agent_a, agent_b) = two_allowlisted_agents().await;

    // A dials B (via B's node id in the endpoint) and asks B's broker to mint
    // an upload URL. B should serve it (A is allowlisted on B).
    let db = "test-db";
    let site = "site-a";
    let upload_ep = format!(
        "{}/v2/cloudsync/databases/{db}/{site}/upload",
        agent_b.address()
    );
    let resp = agent_roundtrip(
        &agent_a.local_addr,
        &Request {
            token: agent_a.token().to_string(),
            endpoint: upload_ep,
            is_post: false,
            body: None,
        },
    )
    .await
    .expect("roundtrip to agent A");

    // A is allowlisted on B, so B serves the upload request → 200 + {"url":...}.
    assert_eq!(resp.status, 200, "allowlisted peer is served");
    let body = String::from_utf8(resp.body.unwrap()).unwrap();
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    let url = v["url"].as_str().unwrap().to_string();
    assert!(
        url.starts_with("mem://"),
        "minted object url is mem://, got {url}"
    );
    // The mem:// URL carries B's node-id fingerprint (the serving peer).
    assert!(
        url.contains(agent_b.node_id().to_z32().as_str()),
        "mem url carries the serving peer's fingerprint: {url}"
    );

    agent_a.stop().await;
    agent_b.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn non_allowlisted_peer_refused_on_dial() {
    // Agent A does NOT allowlist B. A tries to dial B → outbound SSRF gate
    // refuses (403) before any iroh stream is opened.
    let dir_a = tempfile::tempdir().unwrap();
    let dir_b = tempfile::tempdir().unwrap();

    let id_a = Identity::load_or_create_in(dir_a.path()).unwrap();
    let id_b = Identity::load_or_create_in(dir_b.path()).unwrap();

    // A's allowlist is EMPTY — B is not on it.
    let peers_a = PeerStore::load_or_create_in(dir_a.path()).unwrap();
    // B allowlists A so the accept side would otherwise let A in (we're
    // testing the dial gate on A's side here).
    let peers_b = PeerStore::load_or_create_in(dir_b.path()).unwrap();
    peers_b.add_peer(id_a.id(), "A").unwrap();

    let agent_a = P2pAgent::start_with(id_a, peers_a).await.unwrap();
    let agent_b = P2pAgent::start_with(id_b, peers_b).await.unwrap();
    register_direct_addr(agent_a.node_id(), agent_a.direct_addresses()).await;
    register_direct_addr(agent_b.node_id(), agent_b.direct_addresses()).await;
    tokio::time::sleep(Duration::from_millis(100)).await;

    let upload_ep = format!(
        "{}/v2/cloudsync/databases/db/site/upload",
        agent_b.address()
    );
    let resp = agent_roundtrip(
        &agent_a.local_addr,
        &Request {
            token: agent_a.token().to_string(),
            endpoint: upload_ep,
            is_post: false,
            body: None,
        },
    )
    .await
    .unwrap();

    // Outbound SSRF gate: A refuses to dial B (not on A's allowlist) → 403.
    assert_eq!(
        resp.status, 403,
        "non-allowlisted peer refused on dial (outbound SSRF gate, §12)"
    );
    assert!(resp.error.unwrap().contains("not allowlisted"));

    agent_a.stop().await;
    agent_b.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn non_allowlisted_peer_refused_on_accept() {
    // A allowlists B, but B does NOT allowlist A. A dials B (A is allowlisted
    // on... no — A dialing B means A initiates; B's accept gate checks A's
    // node id against B's allowlist. B does NOT allowlist A → B closes the
    // connection (inbound SSRF gate). A's dial succeeds at the QUIC level but
    // the bi-stream read fails / connection is torn down.
    let dir_a = tempfile::tempdir().unwrap();
    let dir_b = tempfile::tempdir().unwrap();

    let id_a = Identity::load_or_create_in(dir_a.path()).unwrap();
    let id_b = Identity::load_or_create_in(dir_b.path()).unwrap();

    let peers_a = PeerStore::load_or_create_in(dir_a.path()).unwrap();
    peers_a.add_peer(id_b.id(), "B").unwrap(); // A will dial B
    // B's allowlist is EMPTY — A is not on it.
    let peers_b = PeerStore::load_or_create_in(dir_b.path()).unwrap();

    let agent_a = P2pAgent::start_with(id_a, peers_a).await.unwrap();
    let agent_b = P2pAgent::start_with(id_b, peers_b).await.unwrap();
    register_direct_addr(agent_a.node_id(), agent_a.direct_addresses()).await;
    register_direct_addr(agent_b.node_id(), agent_b.direct_addresses()).await;
    tokio::time::sleep(Duration::from_millis(100)).await;

    let upload_ep = format!(
        "{}/v2/cloudsync/databases/db/site/upload",
        agent_b.address()
    );
    // A dials B. A passes its own outbound gate (B is allowlisted on A), but
    // B's inbound gate refuses A (A not allowlisted on B) → B closes the conn.
    // A's relay sees a stream/connection error → non-2xx response.
    let resp = agent_roundtrip(
        &agent_a.local_addr,
        &Request {
            token: agent_a.token().to_string(),
            endpoint: upload_ep,
            is_post: false,
            body: None,
        },
    )
    .await
    .unwrap();

    assert!(
        resp.status >= 400,
        "non-allowlisted peer refused on accept (inbound SSRF gate, §12): got status {}",
        resp.status
    );

    agent_a.stop().await;
    agent_b.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn full_collapsed_s3_flow_over_iroh() {
    // The whole 3-step collapsed flow, but peer-to-peer over iroh: A uploads
    // to B, B serves the apply/status/check/download round trip. Proves the
    // broker + iroh relay interoperate end to end.
    let (agent_a, agent_b) = two_allowlisted_agents().await;
    let db = "flow-db";
    let site = "site-a";

    // 1. A → B: upload (GET) → {"url":"mem://<B>/id"}
    let upload_ep = format!(
        "{}/v2/cloudsync/databases/{db}/{site}/upload",
        agent_b.address()
    );
    let resp = agent_roundtrip(
        &agent_a.local_addr,
        &Request {
            token: agent_a.token().to_string(),
            endpoint: upload_ep,
            is_post: false,
            body: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(resp.status, 200);
    let url: String =
        serde_json::from_str::<serde_json::Value>(&String::from_utf8(resp.body.unwrap()).unwrap())
            .unwrap()
            .get("url")
            .unwrap()
            .as_str()
            .unwrap()
            .to_string();
    assert!(url.starts_with("mem://"));

    // 2. A → B: send_buffer (PUT the blob to the mem:// url). Routed to B
    //    because the mem:// url carries B's fingerprint.
    let blob = b"convergence-payload".to_vec();
    let put = PutRequest {
        token: agent_a.token().to_string(),
        url: url.clone(),
        blob,
    };
    // PUT is a different frame shape; send it directly over the agent's TCP.
    let mut stream = TcpStream::connect(&agent_a.local_addr).await.unwrap();
    let json = serde_json::to_vec(&put).unwrap();
    stream
        .write_all(&(json.len() as u32).to_be_bytes())
        .await
        .unwrap();
    stream.write_all(&json).await.unwrap();
    stream.flush().await.unwrap();
    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf).await.unwrap();
    let len = u32::from_be_bytes(len_buf) as usize;
    let mut buf = vec![0u8; len];
    stream.read_exact(&mut buf).await.unwrap();
    let put_resp: serde_json::Value = serde_json::from_slice(&buf).unwrap();
    assert_eq!(put_resp["ok"], true, "PUT to peer's mem:// url succeeds");

    // 3. A → B: apply (POST) → {"lastOptimisticVersion":N,...}
    let apply_body = serde_json::json!({ "url": url, "dbVersionMin": 1, "dbVersionMax": 5 })
        .to_string()
        .into_bytes();
    let apply_ep = format!(
        "{}/v2/cloudsync/databases/{db}/{site}/apply",
        agent_b.address()
    );
    let resp = agent_roundtrip(
        &agent_a.local_addr,
        &Request {
            token: agent_a.token().to_string(),
            endpoint: apply_ep,
            is_post: true,
            body: Some(apply_body),
        },
    )
    .await
    .unwrap();
    assert_eq!(resp.status, 200);
    let v: serde_json::Value = serde_json::from_slice(&resp.body.unwrap()).unwrap();
    assert_eq!(v["lastOptimisticVersion"], 5);

    // 4. A → B: check (POST) → {"url":"mem://..."} (a fresh copy to download)
    let check_body = serde_json::json!({ "dbVersion": 0, "seq": 0 })
        .to_string()
        .into_bytes();
    let check_ep = format!(
        "{}/v2/cloudsync/databases/{db}/{site}/check",
        agent_b.address()
    );
    let resp = agent_roundtrip(
        &agent_a.local_addr,
        &Request {
            token: agent_a.token().to_string(),
            endpoint: check_ep,
            is_post: true,
            body: Some(check_body),
        },
    )
    .await
    .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&resp.body.unwrap()).unwrap();
    let dl_url = v["url"].as_str().unwrap().to_string();
    assert!(dl_url.starts_with("mem://"));

    // 5. A → B: download (GET the mem:// url) → the raw blob bytes.
    let resp = agent_roundtrip(
        &agent_a.local_addr,
        &Request {
            token: agent_a.token().to_string(),
            endpoint: dl_url,
            is_post: false,
            body: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(resp.status, 200);
    assert_eq!(resp.body.unwrap(), b"convergence-payload");

    agent_a.stop().await;
    agent_b.stop().await;
}

/// REGRESSION (audit 2026-08-28, kimi): the inbound allowlist must be
/// re-checked **per bi-stream**, not once per QUIC connection.
///
/// Why this test opens raw streams instead of using `P2pAgent`: the agent's
/// `dial_peer` currently opens a *fresh* connection per request, so the
/// connection-level check alone already refuses a revoked peer — a test driven
/// through the agent passes with or without the fix and proves nothing (this
/// was verified by reverting the fix). The gap is therefore **latent**: the
/// accept loop serves `conn.accept_bi()` in a loop, so the moment SYNC-4 adds
/// connection reuse/pooling (which it should, for efficiency), a peer revoked
/// mid-connection would keep being served until the connection dropped.
///
/// This test pins the invariant now by doing what a pooling client will do:
/// two bi-streams on ONE connection, with the revocation in between. It fails
/// against the pre-fix code.
#[tokio::test]
async fn revoked_peer_is_refused_on_a_reused_connection() {
    let dir_b = tempfile::tempdir().unwrap();
    let id_b = Identity::load_or_create_in(dir_b.path()).unwrap();
    let peers_b = PeerStore::load_or_create_in(dir_b.path()).unwrap();

    // A test-local iroh endpoint plays the peer, so we control its streams.
    let dir_p = tempfile::tempdir().unwrap();
    let id_p = Identity::load_or_create_in(dir_p.path()).unwrap();
    peers_b.add_peer(id_p.id(), "peer").unwrap();

    let agent_b = P2pAgent::start_with(id_b, peers_b.clone()).await.unwrap();
    register_direct_addr(agent_b.node_id(), agent_b.direct_addresses()).await;

    let peer_ep = iroh::Endpoint::builder(iroh::endpoint::presets::Minimal)
        .secret_key(id_p.secret_key().clone())
        .alpns(vec![sync_p2p::agent::SYNC_ALPN.to_vec()])
        .relay_mode(iroh::RelayMode::Disabled)
        .bind_addr("127.0.0.1:0")
        .unwrap()
        .bind()
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;

    let mut addr = iroh::EndpointAddr::new(agent_b.node_id());
    for a in agent_b.direct_addresses() {
        addr = addr.with_ip_addr(a);
    }
    let conn = peer_ep
        .connect(addr, sync_p2p::agent::SYNC_ALPN)
        .await
        .expect("dial B while allowlisted");

    let upload_ep = format!(
        "{}/v2/cloudsync/databases/test-db/site-p/upload",
        agent_b.address()
    );
    let req = Request {
        token: String::new(),
        endpoint: upload_ep,
        is_post: false,
        body: None,
    };

    // Stream 1, while allowlisted: served.
    let (mut send, mut recv) = conn.open_bi().await.expect("open first bi-stream");
    sync_p2p::protocol::write_frame(&mut send, &req)
        .await
        .unwrap();
    send.finish().unwrap();
    let first: Response = sync_p2p::protocol::read_frame(&mut recv).await.unwrap();
    assert_eq!(first.status, 200, "allowlisted peer served on stream 1");

    // Revoke while the SAME connection stays open. Checked so a failed
    // revocation reports itself, rather than surfacing as the less obvious
    // "revoked peer must NOT be served" assertion below.
    peers_b.remove_peer(&id_p.id()).expect("revoke peer");

    // Stream 2 on that same connection must NOT be served.
    let served_after_revocation = match conn.open_bi().await {
        Err(_) => false, // connection torn down — refused
        Ok((mut send2, mut recv2)) => {
            if sync_p2p::protocol::write_frame(&mut send2, &req)
                .await
                .is_err()
            {
                false
            } else {
                let _ = send2.finish();
                match sync_p2p::protocol::read_frame::<_, Response>(&mut recv2).await {
                    Err(_) => false,
                    Ok(r) => r.status == 200,
                }
            }
        }
    };
    assert!(
        !served_after_revocation,
        "revoked peer must NOT be served on a reused connection"
    );

    agent_b.stop().await;
}

/// REGRESSION (SYNC-5): a frame without the bearer token on the C↔agent
/// socket must be refused (401), not served. Before SYNC-5 the localhost TCP
/// port was unauthenticated — any local process that could reach it could
/// read/write sync data, bypassing the peer allowlist from the local side.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn missing_token_is_refused_on_c_socket() {
    let dir = tempfile::tempdir().unwrap();
    let id = Identity::load_or_create_in(dir.path()).unwrap();
    let peers = PeerStore::load_or_create_in(dir.path()).unwrap();
    let agent = P2pAgent::start_with(id, peers).await.unwrap();

    let upload_ep = format!("{}/v2/cloudsync/databases/db/site/upload", agent.address());
    let resp = agent_roundtrip(
        &agent.local_addr,
        &Request {
            token: String::new(),
            endpoint: upload_ep,
            is_post: false,
            body: None,
        },
    )
    .await
    .unwrap();

    assert_eq!(resp.status, 401, "token-less frame must be refused");
    assert!(resp.error.unwrap().contains("invalid sync token"));
    assert!(resp.body.is_none(), "401 must serve nothing");

    agent.stop().await;
}

/// REGRESSION (SYNC-5): wrong token refused, receive shape.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn wrong_token_is_refused_on_c_socket_receive() {
    let dir = tempfile::tempdir().unwrap();
    let id = Identity::load_or_create_in(dir.path()).unwrap();
    let peers = PeerStore::load_or_create_in(dir.path()).unwrap();
    let agent = P2pAgent::start_with(id, peers).await.unwrap();

    let upload_ep = format!("{}/v2/cloudsync/databases/db/site/upload", agent.address());
    let resp = agent_roundtrip(
        &agent.local_addr,
        &Request {
            token: "deadbeefdeadbeefdeadbeefdeadbeef".into(),
            endpoint: upload_ep,
            is_post: false,
            body: None,
        },
    )
    .await
    .unwrap();

    assert_eq!(resp.status, 401, "wrong token must be refused");
    assert!(resp.body.is_none(), "401 must serve nothing");

    agent.stop().await;
}

/// REGRESSION (SYNC-5): wrong token refused, PUT shape (`PutResponse{ok:false}`).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn wrong_token_is_refused_on_c_socket_put() {
    let dir = tempfile::tempdir().unwrap();
    let id = Identity::load_or_create_in(dir.path()).unwrap();
    let peers = PeerStore::load_or_create_in(dir.path()).unwrap();
    let agent = P2pAgent::start_with(id, peers).await.unwrap();

    let put = PutRequest {
        token: "not-the-token".into(),
        url: format!("{}/v2/cloudsync/databases/db/site/upload", agent.address()),
        blob: b"payload".to_vec(),
    };
    let mut stream = TcpStream::connect(&agent.local_addr).await.unwrap();
    let json = serde_json::to_vec(&put).unwrap();
    stream
        .write_all(&(json.len() as u32).to_be_bytes())
        .await
        .unwrap();
    stream.write_all(&json).await.unwrap();
    stream.flush().await.unwrap();
    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf).await.unwrap();
    let len = u32::from_be_bytes(len_buf) as usize;
    let mut buf = vec![0u8; len];
    stream.read_exact(&mut buf).await.unwrap();
    let put_resp: serde_json::Value = serde_json::from_slice(&buf).unwrap();

    assert_eq!(put_resp["ok"], false, "wrong token must be refused (PUT)");
    assert!(
        put_resp["error"]
            .as_str()
            .unwrap()
            .contains("invalid sync token")
    );

    agent.stop().await;
}

/// The C↔agent bearer token is NOT required on the inbound iroh path: a peer
/// frame (which never carries a token) is still served. Guards against
/// over-tightening SYNC-5 into the peer path (the wire format over iroh does
/// not change).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn token_is_not_required_on_the_iroh_peer_path() {
    let (agent_a, agent_b) = two_allowlisted_agents().await;

    let upload_ep = format!(
        "{}/v2/cloudsync/databases/db/site/upload",
        agent_b.address()
    );
    let resp = agent_roundtrip(
        &agent_a.local_addr,
        &Request {
            // Correct token → the agent dials B over iroh. B serves A's frame,
            // which has NO token of its own (iroh frames never carry one).
            token: agent_a.token().to_string(),
            endpoint: upload_ep,
            is_post: false,
            body: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(
        resp.status, 200,
        "peer path must work without a token (iroh frames never carry one)"
    );

    agent_a.stop().await;
    agent_b.stop().await;
}

/// Requirement 4 / §15.2 "offline reconnect" gate item: a peer that goes
/// offline mid-session must fail a dial **bounded and cleanly** — a clear
/// error, no hang, no panic — and the failure must not poison later attempts.
/// When the peer comes back (same identity, same node id — a real device
/// returning, not a re-pairing), the next sync tick reconnects and converges.
/// Driven entirely in `Loopback` mode so it is deterministic and needs no
/// network; the offline-ness is real (B's iroh endpoint and TCP listener are
/// actually stopped), only the transport is loopback.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn dial_to_offline_peer_fails_bounded_then_reconnects_after_peer_returns() {
    let dir_a = tempfile::tempdir().unwrap();
    let dir_b = tempfile::tempdir().unwrap();

    let id_a = Identity::load_or_create_in(dir_a.path()).unwrap();
    let id_b = Identity::load_or_create_in(dir_b.path()).unwrap();

    let peers_a = PeerStore::load_or_create_in(dir_a.path()).unwrap();
    let peers_b = PeerStore::load_or_create_in(dir_b.path()).unwrap();
    peers_a.add_peer(id_b.id(), "B").unwrap();
    peers_b.add_peer(id_a.id(), "A").unwrap();

    let agent_a = P2pAgent::start_with(id_a, peers_a).await.unwrap();
    let agent_b = P2pAgent::start_with(id_b.clone(), peers_b.clone())
        .await
        .unwrap();
    register_direct_addr(agent_a.node_id(), agent_a.direct_addresses()).await;
    register_direct_addr(agent_b.node_id(), agent_b.direct_addresses()).await;
    tokio::time::sleep(Duration::from_millis(100)).await;

    let db = "reconnect-db";
    let site = "site-a";
    // B's address is a function of its node id, which does not change across
    // the stop/restart below, so this endpoint URL stays valid throughout.
    let upload_ep = format!(
        "{}/v2/cloudsync/databases/{db}/{site}/upload",
        agent_b.address()
    );

    // 1. B is up: the first request reaches it.
    let resp = agent_roundtrip(
        &agent_a.local_addr,
        &Request {
            token: agent_a.token().to_string(),
            endpoint: upload_ep.clone(),
            is_post: false,
            body: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(resp.status, 200, "B initially reachable");

    // 2. B goes offline: stop its agent (closes its iroh endpoint and TCP
    //    listener). The stale direct address stays registered — exactly what
    //    a real disconnect looks like, since nothing tells A's side to clear
    //    the entry when a peer drops.
    agent_b.stop().await;

    // 3. The next request must fail bounded and cleanly: no hang, no panic, a
    //    clear error status. The generous outer timeout is a safety valve so
    //    a regression that reintroduces an unbounded retry fails this test
    //    instead of hanging the whole suite.
    let offline_resp = tokio::time::timeout(
        Duration::from_secs(10),
        agent_roundtrip(
            &agent_a.local_addr,
            &Request {
                token: agent_a.token().to_string(),
                endpoint: upload_ep.clone(),
                is_post: false,
                body: None,
            },
        ),
    )
    .await
    .expect("dial to an offline peer must not hang")
    .unwrap();
    assert_eq!(
        offline_resp.status, 502,
        "dial to an offline peer fails cleanly with a clear error, not silently"
    );
    assert!(offline_resp.error.is_some());

    // 4. B returns: a fresh agent on the SAME identity (same node id — this
    //    is the device coming back, not a new pairing). Its new direct
    //    address is (re-)registered, exactly as real discovery would
    //    republish an updated address record.
    let agent_b2 = P2pAgent::start_with(id_b, peers_b).await.unwrap();
    register_direct_addr(agent_b2.node_id(), agent_b2.direct_addresses()).await;
    tokio::time::sleep(Duration::from_millis(100)).await;

    // 5. The next sync tick reconnects and converges — same request as step
    //    1, succeeding again. This also proves step 3's failure did not
    //    poison later attempts (no negative caching, no stuck error state).
    let resp = agent_roundtrip(
        &agent_a.local_addr,
        &Request {
            token: agent_a.token().to_string(),
            endpoint: upload_ep,
            is_post: false,
            body: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(
        resp.status, 200,
        "reconnect after peer returns must succeed"
    );

    agent_a.stop().await;
    agent_b2.stop().await;
}

/// SYNC-8 end-to-end proof: two agents in **`AgentTransport::Discovered`**
/// find each other by node id alone, over iroh's real relay/DNS-pkarr
/// address-lookup service against n0's live infrastructure. This is the
/// actual capability SYNC-8 exists to deliver, and every other test in this
/// crate proves it only indirectly (`Loopback` mode's `register_direct_addr`
/// registry stands in for discovery everywhere else). `register_direct_addr`
/// is deliberately never called here.
///
/// **Needs real network egress and a live n0.computer.** Not run by default —
/// `cargo test -p sync-p2p` must stay fully offline and deterministic, and
/// n0's infrastructure and DNS/pkarr propagation delay are neither. Run it
/// explicitly:
///
/// ```text
/// cargo test -p sync-p2p -- --ignored discovered_mode_dials_by_node_id_alone_over_real_network
/// ```
#[ignore = "needs real network egress + n0's live DNS/pkarr infrastructure — see doc comment"]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn discovered_mode_dials_by_node_id_alone_over_real_network() {
    let dir_a = tempfile::tempdir().unwrap();
    let dir_b = tempfile::tempdir().unwrap();

    let id_a = Identity::load_or_create_in(dir_a.path()).unwrap();
    let id_b = Identity::load_or_create_in(dir_b.path()).unwrap();

    let peers_a = PeerStore::load_or_create_in(dir_a.path()).unwrap();
    let peers_b = PeerStore::load_or_create_in(dir_b.path()).unwrap();
    peers_a.add_peer(id_b.id(), "B").unwrap();
    peers_b.add_peer(id_a.id(), "A").unwrap();

    let agent_a = P2pAgent::start_with_transport(id_a, peers_a, AgentTransport::Discovered)
        .await
        .unwrap();
    let agent_b = P2pAgent::start_with_transport(id_b, peers_b, AgentTransport::Discovered)
        .await
        .unwrap();

    // Deliberately NO register_direct_addr call anywhere in this test: A must
    // find B purely through iroh's address-lookup service (pkarr publish by
    // B, resolved by A via n0's DNS/pkarr relay), not the Loopback-only
    // process-local registry every other test in this file relies on.

    // Give B's PkarrPublisher time to push its initial address record and let
    // that propagate through n0's relay before A tries to resolve it.
    tokio::time::sleep(Duration::from_secs(3)).await;

    let db = "discovered-e2e-db";
    let site = "site-a";
    let upload_ep = format!(
        "{}/v2/cloudsync/databases/{db}/{site}/upload",
        agent_b.address()
    );

    // Outer bound so a genuine failure (DNS blocked, egress blocked, n0 down)
    // reports as a clear timeout rather than hanging the run.
    let resp = tokio::time::timeout(
        Duration::from_secs(30),
        agent_roundtrip(
            &agent_a.local_addr,
            &Request {
                token: agent_a.token().to_string(),
                endpoint: upload_ep,
                is_post: false,
                body: None,
            },
        ),
    )
    .await
    .expect("dial over real discovery must not hang past the outer bound")
    .unwrap();

    assert_eq!(
        resp.status, 200,
        "A must reach B by node id alone via real relay/DNS discovery, got: {resp:?}"
    );
    let body = String::from_utf8(resp.body.unwrap()).unwrap();
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    let url = v["url"].as_str().unwrap().to_string();
    assert!(
        url.starts_with("mem://"),
        "minted object url is mem://, got {url}"
    );

    // Round-trip an actual payload through the discovered connection: PUT the
    // blob, then GET it back.
    let blob = b"sync-8-real-discovery-payload".to_vec();
    let put = PutRequest {
        token: agent_a.token().to_string(),
        url: url.clone(),
        blob: blob.clone(),
    };
    let mut stream = TcpStream::connect(&agent_a.local_addr).await.unwrap();
    let json = serde_json::to_vec(&put).unwrap();
    stream
        .write_all(&(json.len() as u32).to_be_bytes())
        .await
        .unwrap();
    stream.write_all(&json).await.unwrap();
    stream.flush().await.unwrap();
    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf).await.unwrap();
    let len = u32::from_be_bytes(len_buf) as usize;
    let mut buf = vec![0u8; len];
    stream.read_exact(&mut buf).await.unwrap();
    let put_resp: serde_json::Value = serde_json::from_slice(&buf).unwrap();
    assert_eq!(
        put_resp["ok"], true,
        "PUT over a real-discovery connection must succeed: {put_resp:?}"
    );

    let resp = agent_roundtrip(
        &agent_a.local_addr,
        &Request {
            token: agent_a.token().to_string(),
            endpoint: url,
            is_post: false,
            body: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(resp.status, 200);
    assert_eq!(
        resp.body.unwrap(),
        blob,
        "payload round-trips over the discovered connection"
    );

    agent_a.stop().await;
    agent_b.stop().await;
}

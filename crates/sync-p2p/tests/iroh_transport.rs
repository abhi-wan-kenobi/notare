//! Integration tests for the iroh P2P transport + allowlist enforcement.
//!
//! These drive two [`P2pAgent`]s against each other over real iroh/QUIC
//! (loopback, relay disabled) and assert:
//!   - an allowlisted peer can be dialed and served (the relay path works);
//!   - a non-allowlisted node id is refused on the **dial** side (outbound
//!     SSRF gate, §12) — the agent returns a 403 and never opens a stream;
//!   - a non-allowlisted node id is refused on the **accept** side (inbound
//!     SSRF gate, §12) — the connection is closed with no streams served.
//!
//! Same-machine only (both agents in one process); production NAT traversal
//! is iroh's job and out of scope for this PR.

use std::time::Duration;

use sync_p2p::{Identity, PeerStore, P2pAgent, register_direct_addr};
use sync_p2p::protocol::{PutRequest, Request, Response};

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
    let upload_ep = format!("{}/v2/cloudsync/databases/{db}/{site}/upload", agent_b.address());
    let resp = agent_roundtrip(
        &agent_a.local_addr,
        &Request { endpoint: upload_ep, is_post: false, body: None },
    )
    .await
    .unwrap();
    assert_eq!(resp.status, 200);
    let url: String = serde_json::from_str::<serde_json::Value>(&String::from_utf8(resp.body.unwrap()).unwrap())
        .unwrap()
        .get("url").unwrap().as_str().unwrap().to_string();
    assert!(url.starts_with("mem://"));

    // 2. A → B: send_buffer (PUT the blob to the mem:// url). Routed to B
    //    because the mem:// url carries B's fingerprint.
    let blob = b"convergence-payload".to_vec();
    let put = PutRequest { url: url.clone(), blob };
    // PUT is a different frame shape; send it directly over the agent's TCP.
    let mut stream = TcpStream::connect(&agent_a.local_addr).await.unwrap();
    let json = serde_json::to_vec(&put).unwrap();
    stream.write_all(&(json.len() as u32).to_be_bytes()).await.unwrap();
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
        .to_string().into_bytes();
    let apply_ep = format!("{}/v2/cloudsync/databases/{db}/{site}/apply", agent_b.address());
    let resp = agent_roundtrip(
        &agent_a.local_addr,
        &Request { endpoint: apply_ep, is_post: true, body: Some(apply_body) },
    )
    .await
    .unwrap();
    assert_eq!(resp.status, 200);
    let v: serde_json::Value = serde_json::from_slice(&resp.body.unwrap()).unwrap();
    assert_eq!(v["lastOptimisticVersion"], 5);

    // 4. A → B: check (POST) → {"url":"mem://..."} (a fresh copy to download)
    let check_body = serde_json::json!({ "dbVersion": 0, "seq": 0 }).to_string().into_bytes();
    let check_ep = format!("{}/v2/cloudsync/databases/{db}/{site}/check", agent_b.address());
    let resp = agent_roundtrip(
        &agent_a.local_addr,
        &Request { endpoint: check_ep, is_post: true, body: Some(check_body) },
    )
    .await
    .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&resp.body.unwrap()).unwrap();
    let dl_url = v["url"].as_str().unwrap().to_string();
    assert!(dl_url.starts_with("mem://"));

    // 5. A → B: download (GET the mem:// url) → the raw blob bytes.
    let resp = agent_roundtrip(
        &agent_a.local_addr,
        &Request { endpoint: dl_url, is_post: false, body: None },
    )
    .await
    .unwrap();
    assert_eq!(resp.status, 200);
    assert_eq!(resp.body.unwrap(), b"convergence-payload");

    agent_a.stop().await;
    agent_b.stop().await;
}

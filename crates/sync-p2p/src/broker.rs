//! The local "CloudSync server" — a stand-in for SQLite Cloud / S3 that serves
//! the CloudSync control protocol directly to peers over TCP, collapsing the
//! S3 pre-signed-URL 3-step flow onto an in-memory object store.
//!
//! In production this server is `https://cloudsync.sqlite.ai` + S3. For the S1
//! spike it is a localhost tokio TCP server holding, per managed-database ID:
//!   - the most recently **uploaded** changes blob (the object an S3 PUT would
//!     land in object storage), plus its `dbVersionMin`/`dbVersionMax` range;
//!   - a monotonically increasing `lastOptimisticVersion` counter.
//!
//! The four cloudsync endpoints map onto it as follows (see the contract doc,
//! §9.8/§9.9, and the S1 call-graph appendix):
//!
//! | core call                      | broker action                                   |
//! |-------------------------------|-------------------------------------------------|
//! | `receive(upload, GET)`         | mint a `mem://` URL, return `{"url":...}`        |
//! | `send_buffer(mem://, PUT blob)`| store the blob under that URL                    |
//! | `receive(apply, POST)`        | record the upload, bump optimistic version,      |
//! |                               | return `{"lastOptimisticVersion":...,"gaps":[]}`|
//! | `receive(check, POST)`        | if a newer blob exists, return `{"url":"mem://..."}` |
//! | `receive(mem://, GET)`         | return the stored blob as the changes payload     |
//! | `receive(status, GET)`        | return `{"lastOptimisticVersion":...}`           |
//!
//! The blob is opaque to the broker — it is the sqlite-sync changes payload
//! (an encoded CRDT changeset) produced by the uploader's
//! `cloudsync_payload_encode` and consumed by the downloader's
//! `cloudsync_payload_apply`. The CRDT merge happens inside the sqlite-sync
//! core on each peer; the broker only shuttles bytes.

use std::collections::HashMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::io::AsyncReadExt;
use tokio::net::TcpListener;
use tokio::sync::Mutex;

use crate::protocol::{PutRequest, PutResponse, Request, Response, write_frame};

/// A stored changes blob + the version range it covers, plus a broker-assigned
/// monotonic sequence so per-site delivery can track it.
#[derive(Debug, Clone)]
struct StoredBlob {
    seq: u64,
    bytes: Vec<u8>,
    // Retained for debugging/diagnostics; the spike's check path uses per-site
    // delivery (seq) rather than the per-site-local db_version range.
    #[allow(dead_code)]
    db_version_min: i64,
    #[allow(dead_code)]
    db_version_max: i64,
}

/// Per-database state held by the broker.
#[derive(Debug, Default)]
struct DbState {
    /// ALL uploaded changes blobs, in upload order (the "object log"). The
    /// real CloudSync server keeps a global ordered change log indexed by a
    /// server-assigned db_version; the spike approximates it by retaining every
    /// uploaded blob and serving each site the ones it hasn't pulled yet.
    blobs: Vec<StoredBlob>,
    /// Per-site high-water mark: the broker sequence each site has pulled up
    /// to. Drives "what's new for this site" — the core's own db_version is
    /// per-site-local and cannot order cross-site changes, so the broker tracks
    /// delivery per site instead.
    delivered: HashMap<String, u64>,
    /// Monotonically increasing; mirrors the server's `lastOptimisticVersion`.
    last_optimistic_version: i64,
    last_confirmed_version: i64,
    /// Next broker-assigned blob sequence.
    next_seq: u64,
}

/// The broker's shared state.
#[derive(Debug)]
struct BrokerState {
    /// `managedDatabaseId -> state`.
    dbs: Mutex<HashMap<String, DbState>>,
    /// `mem://addr/<id> -> blob bytes` object store (the S3 bucket), keyed by
    /// the full mem:// URL the broker minted.
    objects: Mutex<HashMap<String, Vec<u8>>>,
    /// The broker's own `host:port`, embedded in minted mem:// URLs so the C
    /// `network_send_buffer` can connect back to the right broker.
    addr: String,
}

impl Default for BrokerState {
    fn default() -> Self {
        Self {
            dbs: Mutex::new(HashMap::new()),
            objects: Mutex::new(HashMap::new()),
            addr: String::new(),
        }
    }
}

/// A running broker — a localhost TCP server speaking the framed protocol.
pub struct Broker {
    pub addr: String,
    // Kept to extend the shared state's lifetime to the broker's; the accept
    // loop holds its own clone, so this is not read directly.
    #[allow(dead_code)]
    state: Arc<BrokerState>,
    handle: tokio::task::JoinHandle<()>,
}

/// Parsed endpoint: `p2p://host:port/<dbId>/<siteId>/<action>`.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Endpoint {
    action: String,
    db_id: String,
    _site_id: String,
}

fn parse_endpoint(url: &str) -> Option<Endpoint> {
    // The core builds `{address}/v2/cloudsync/databases/{dbId}/{siteId}/{action}`
    // where `address` is the `p2p://host:port` we set via cloudsync_network_init_custom.
    // We only care about the trailing three path segments.
    let after_scheme = url
        .strip_prefix("p2p://")
        .or_else(|| url.strip_prefix("http://"))?;
    // Drop the `/v2/cloudsync/databases/` prefix if present (the core adds it).
    let path = after_scheme
        .split_once('/')
        .map(|(_, rest)| rest)
        .unwrap_or(after_scheme);
    let path = path.strip_prefix("v2/cloudsync/databases/").unwrap_or(path);
    let segs: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    if segs.len() < 3 {
        return None;
    }
    let n = segs.len();
    Some(Endpoint {
        db_id: segs[n - 3].to_string(),
        _site_id: segs[n - 2].to_string(),
        action: segs[n - 1].to_string(),
    })
}

/// A JSON body the core POSTs to the apply endpoint.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApplyBody {
    url: String,
    #[serde(default)]
    db_version_min: i64,
    #[serde(default)]
    db_version_max: i64,
}

/// A JSON body the core POSTs to the check endpoint.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CheckBody {
    #[serde(default)]
    db_version: i64,
    #[allow(dead_code)]
    #[serde(default)]
    seq: i64,
}

fn json_ok<T: Serialize>(value: &T) -> Response {
    Response {
        status: 200,
        body: Some(serde_json::to_vec(value).unwrap_or_default()),
        error: None,
    }
}

fn json_err(msg: impl Into<String>) -> Response {
    Response {
        status: 500,
        body: None,
        error: Some(msg.into()),
    }
}

fn mint_object_url(broker_addr: &str) -> String {
    // A unique mem:// URL for the uploaded blob, carrying the broker's
    // host:port so the C `network_send_buffer` (which receives this URL with
    // no other context) can connect back to the right broker. In S3 terms this
    // is the pre-signed PUT URL; here it is a key into the in-memory store.
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let id = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("mem://{broker_addr}/{id}")
}

impl BrokerState {
    async fn handle_request(&self, req: Request) -> Response {
        // Download URL: a `mem://<id>` the core GETs via network_download_changes.
        // Route it before parse_endpoint (which only matches the 3-segment
        // /dbId/siteId/action control endpoints).
        if req.endpoint.starts_with("mem://") {
            let key = req.endpoint.to_string();
            let objects = self.objects.lock().await;
            return match objects.get(&key) {
                Some(bytes) => Response {
                    status: 200,
                    body: Some(bytes.clone()),
                    error: None,
                },
                None => json_err(format!("download: unknown object url: {key}")),
            };
        }

        let Some(ep) = parse_endpoint(&req.endpoint) else {
            return json_err(format!("unparseable endpoint: {}", req.endpoint));
        };

        match ep.action.as_str() {
            "upload" => {
                // GET: hand back a URL the core will PUT the blob to.
                let url = mint_object_url(&self.addr);
                json_ok(&serde_json::json!({ "url": url }))
            }
            "apply" => {
                // POST: the core tells us it finished uploading; record it.
                let body = match req.body {
                    Some(b) => b,
                    None => return json_err("apply: missing body"),
                };
                let ApplyBody {
                    url,
                    db_version_min,
                    db_version_max,
                } = match serde_json::from_slice(&body) {
                    Ok(v) => v,
                    Err(e) => return json_err(format!("apply: bad json: {e}")),
                };
                let bytes = {
                    let objects = self.objects.lock().await;
                    match objects.get(&url) {
                        Some(b) => b.clone(),
                        None => return json_err(format!("apply: unknown object url: {url}")),
                    }
                };
                let mut dbs = self.dbs.lock().await;
                let state = dbs.entry(ep.db_id).or_default();
                state.next_seq += 1;
                let seq = state.next_seq; // starts at 1 so the first blob > delivered(0)
                state.blobs.push(StoredBlob {
                    seq,
                    bytes,
                    db_version_min,
                    db_version_max,
                });
                state.last_optimistic_version = state.last_optimistic_version.max(db_version_max);
                state.last_confirmed_version = state.last_confirmed_version.max(db_version_max);
                json_ok(&serde_json::json!({
                    "lastOptimisticVersion": state.last_optimistic_version,
                    "lastConfirmedVersion": state.last_confirmed_version,
                    "gaps": [],
                }))
            }
            "check" => {
                // POST {"dbVersion":N,"seq":S} from a site. The core's db_version
                // is per-site-local and cannot order cross-site changes, so the
                // broker ignores it and tracks delivery per site instead: serve
                // the earliest blob this site hasn't pulled yet (its high-water
                // mark in the broker's upload log).
                let _db_version = match req.body.as_ref() {
                    Some(b) => serde_json::from_slice::<CheckBody>(b)
                        .map(|c| c.db_version)
                        .unwrap_or(0),
                    None => 0,
                };
                let mut dbs = self.dbs.lock().await;
                let Some(state) = dbs.get_mut(&ep.db_id) else {
                    // No uploads yet — no changes to pull. Return OK with no
                    // body (204): the core's check_internal treats a non-BUFFER
                    // response as "no changes" (network_set_sqlite_result),
                    // avoiding the "missing 'url'" error path.
                    return Response {
                        status: 204,
                        body: None,
                        error: None,
                    };
                };
                let delivered = state.delivered.entry(ep._site_id.clone()).or_insert(0);
                // Serve the smallest-seq blob strictly greater than the site's
                // high-water mark. (delivered starts at 0; the first blob is seq 0
                // and `> 0` is false, so start blobs at seq 1 via next_seq init.)
                let next = state.blobs.iter().find(|b| b.seq > *delivered);
                let Some(blob) = next else {
                    // This site is up to date.
                    return Response {
                        status: 204,
                        body: None,
                        error: None,
                    };
                };
                *delivered = blob.seq;
                let bytes = blob.bytes.clone();
                drop(dbs);
                // Re-store the blob under a fresh mem:// key for the downloader.
                let url = mint_object_url(&self.addr);
                {
                    let mut objects = self.objects.lock().await;
                    objects.insert(url.clone(), bytes);
                }
                json_ok(&serde_json::json!({ "url": url }))
            }
            "status" => {
                // GET: report the optimistic/confirmed versions.
                let dbs = self.dbs.lock().await;
                let state = dbs.get(&ep.db_id);
                let (optv, conv) = match state {
                    Some(s) => (s.last_optimistic_version, s.last_confirmed_version),
                    None => (0, 0),
                };
                json_ok(&serde_json::json!({
                    "lastOptimisticVersion": optv,
                    "lastConfirmedVersion": conv,
                    "gaps": [],
                }))
            }
            other => json_err(format!("unknown action: {other}")),
        }
    }
}

impl Broker {
    /// Start a broker listening on an ephemeral localhost port.
    pub async fn start() -> std::io::Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let addr = listener.local_addr()?.to_string();
        let state = Arc::new(BrokerState {
            addr: addr.clone(),
            ..Default::default()
        });

        let state_clone = Arc::clone(&state);
        let handle = tokio::spawn(async move {
            loop {
                let (mut stream, _) = match listener.accept().await {
                    Ok(s) => s,
                    Err(_) => break,
                };
                let state = Arc::clone(&state_clone);
                tokio::spawn(async move {
                    // One frame in, one frame out. A connection may carry either a
                    // `Request` (receive_buffer) or a `PutRequest` (send_buffer);
                    // distinguish by peeking the JSON shape.
                    let mut buf = Vec::new();
                    if read_into(&mut stream, &mut buf).await.is_err() {
                        return;
                    }
                    // Try Request first; the `endpoint` field disambiguates.
                    if let Ok(req) = serde_json::from_slice::<Request>(&buf) {
                        let resp = state.handle_request(req).await;
                        let _ = write_frame(&mut stream, &resp).await;
                        return;
                    }
                    if let Ok(put) = serde_json::from_slice::<PutRequest>(&buf) {
                        let mut objects = state.objects.lock().await;
                        objects.insert(put.url.clone(), put.blob);
                        drop(objects);
                        let resp = PutResponse {
                            ok: true,
                            error: None,
                        };
                        let _ = write_frame(&mut stream, &resp).await;
                        return;
                    }
                    let resp = PutResponse {
                        ok: false,
                        error: Some("unparseable frame".into()),
                    };
                    let _ = write_frame(&mut stream, &resp).await;
                });
            }
        });

        Ok(Self {
            addr,
            state,
            handle,
        })
    }

    /// Connect address for sites (`p2p://<addr>`).
    pub fn address(&self) -> String {
        format!("p2p://{}", self.addr)
    }

    pub async fn stop(self) {
        self.handle.abort();
    }
}

/// Read one length-prefixed frame into `buf`.
async fn read_into<R: AsyncReadExt + Unpin>(r: &mut R, buf: &mut Vec<u8>) -> std::io::Result<()> {
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

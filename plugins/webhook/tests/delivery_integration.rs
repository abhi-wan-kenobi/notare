//! Integration tests for outbound webhook delivery against a real local HTTP
//! receiver. Exercises signature correctness, payload round-trip, retry on
//! transient 5xx, and no-retry on permanent 4xx.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::Mutex;

use tauri_plugin_webhook::delivery::{RetryPolicy, WebhookEnvelope, deliver, sign_body};

/// What the fake receiver captured from a single request.
#[derive(Clone, Default)]
struct Captured {
    signature: Option<String>,
    event_header: Option<String>,
    body: Vec<u8>,
}

/// A local HTTP receiver that returns a scripted sequence of status codes
/// (one per request), capturing each request's signature header and body.
struct FakeReceiver {
    addr: std::net::SocketAddr,
    captures: Arc<Mutex<Vec<Captured>>>,
    hits: Arc<AtomicUsize>,
}

async fn spawn_receiver(statuses: Vec<u16>) -> FakeReceiver {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let captures = Arc::new(Mutex::new(Vec::<Captured>::new()));
    let hits = Arc::new(AtomicUsize::new(0));

    let captures_task = captures.clone();
    let hits_task = hits.clone();
    tokio::spawn(async move {
        loop {
            let Ok((mut stream, _)) = listener.accept().await else {
                break;
            };
            let idx = hits_task.fetch_add(1, Ordering::SeqCst);
            let status = *statuses.get(idx).unwrap_or(statuses.last().unwrap());

            // Read headers up to CRLFCRLF, then the Content-Length body.
            let mut buf = Vec::new();
            let mut tmp = [0u8; 1024];
            let header_end = loop {
                let n = stream.read(&mut tmp).await.unwrap_or(0);
                if n == 0 {
                    break None;
                }
                buf.extend_from_slice(&tmp[..n]);
                if let Some(pos) = find_subslice(&buf, b"\r\n\r\n") {
                    break Some(pos);
                }
            };

            let mut cap = Captured::default();
            if let Some(header_end) = header_end {
                let head = String::from_utf8_lossy(&buf[..header_end]).to_string();
                let mut content_length = 0usize;
                for line in head.lines() {
                    let lower = line.to_ascii_lowercase();
                    if let Some(v) = lower.strip_prefix("content-length:") {
                        content_length = v.trim().parse().unwrap_or(0);
                    }
                    if let Some(v) = line.splitn(2, ':').nth(1) {
                        if lower.starts_with("x-notare-signature:") {
                            cap.signature = Some(v.trim().to_string());
                        } else if lower.starts_with("x-notare-event:") {
                            cap.event_header = Some(v.trim().to_string());
                        }
                    }
                }

                let body_start = header_end + 4;
                let mut body = buf[body_start..].to_vec();
                while body.len() < content_length {
                    let n = stream.read(&mut tmp).await.unwrap_or(0);
                    if n == 0 {
                        break;
                    }
                    body.extend_from_slice(&tmp[..n]);
                }
                body.truncate(content_length);
                cap.body = body;
            }

            captures_task.lock().await.push(cap);

            let response =
                format!("HTTP/1.1 {status} X\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
            let _ = stream.write_all(response.as_bytes()).await;
            let _ = stream.flush().await;
        }
    });

    FakeReceiver {
        addr,
        captures,
        hits,
    }
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

fn fast_policy(max_attempts: u32) -> RetryPolicy {
    RetryPolicy {
        max_attempts,
        base_delay: Duration::from_millis(5),
        max_delay: Duration::from_millis(20),
    }
}

fn envelope(event_type: &str, data: serde_json::Value) -> WebhookEnvelope {
    WebhookEnvelope {
        id: "test-delivery-id".to_string(),
        event_type: event_type.to_string(),
        timestamp: "2026-07-22T00:00:00Z".to_string(),
        data,
    }
}

#[tokio::test]
async fn delivers_and_signs_body_correctly() {
    let recv = spawn_receiver(vec![200]).await;
    let url = format!("http://{}/hook", recv.addr);
    let secret = "shared-secret";
    let client = reqwest::Client::new();
    let env = envelope(
        "action_items.updated",
        serde_json::json!({ "items": ["ship it"] }),
    );

    let record = deliver(&client, &url, secret, &env, fast_policy(3)).await;

    assert!(record.success, "expected success, got {record:?}");
    assert_eq!(record.status_code, Some(200));
    assert_eq!(record.attempts, 1);

    let caps = recv.captures.lock().await;
    assert_eq!(caps.len(), 1);
    let cap = &caps[0];
    assert_eq!(cap.event_header.as_deref(), Some("action_items.updated"));

    // The captured body must verify against the shared secret, and match the
    // exact signature we advertised.
    let expected_sig = sign_body(secret, &cap.body);
    assert_eq!(cap.signature.as_deref(), Some(expected_sig.as_str()));
    assert!(expected_sig.starts_with("sha256="));

    // And the body is the JSON envelope carrying our payload.
    let parsed: serde_json::Value = serde_json::from_slice(&cap.body).unwrap();
    assert_eq!(parsed["event_type"], "action_items.updated");
    assert_eq!(parsed["data"]["items"][0], "ship it");
}

#[tokio::test]
async fn retries_transient_5xx_then_succeeds() {
    // 503, 503, then 200 -> should take exactly 3 attempts and succeed.
    let recv = spawn_receiver(vec![503, 503, 200]).await;
    let url = format!("http://{}/hook", recv.addr);
    let client = reqwest::Client::new();
    let env = envelope("session.enhanced", serde_json::json!({ "ok": true }));

    let record = deliver(&client, &url, "s", &env, fast_policy(3)).await;

    assert!(record.success, "expected eventual success, got {record:?}");
    assert_eq!(record.status_code, Some(200));
    assert_eq!(record.attempts, 3);
    assert_eq!(recv.hits.load(Ordering::SeqCst), 3);
}

#[tokio::test]
async fn does_not_retry_permanent_4xx() {
    let recv = spawn_receiver(vec![400, 400, 400]).await;
    let url = format!("http://{}/hook", recv.addr);
    let client = reqwest::Client::new();
    let env = envelope("session.enhanced", serde_json::json!({}));

    let record = deliver(&client, &url, "s", &env, fast_policy(3)).await;

    assert!(!record.success);
    assert_eq!(record.status_code, Some(400));
    assert_eq!(record.attempts, 1, "4xx must not be retried");
    assert_eq!(recv.hits.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn exhausts_attempts_on_persistent_5xx() {
    let recv = spawn_receiver(vec![500]).await;
    let url = format!("http://{}/hook", recv.addr);
    let client = reqwest::Client::new();
    let env = envelope("session.enhanced", serde_json::json!({}));

    let record = deliver(&client, &url, "s", &env, fast_policy(3)).await;

    assert!(!record.success);
    assert_eq!(record.status_code, Some(500));
    assert_eq!(record.attempts, 3);
    assert_eq!(recv.hits.load(Ordering::SeqCst), 3);
}

#[tokio::test]
async fn retries_then_fails_on_connection_refused() {
    // Bind + immediately drop to get an almost-certainly-closed port.
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    drop(listener);

    let url = format!("http://{addr}/hook");
    let client = reqwest::Client::new();
    let env = envelope("session.enhanced", serde_json::json!({}));

    let record = deliver(&client, &url, "s", &env, fast_policy(3)).await;

    assert!(!record.success);
    assert!(record.error.is_some());
    // Connection errors are transient => all attempts consumed.
    assert_eq!(record.attempts, 3);
}

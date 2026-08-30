//! Protocol-level integration test: drives the broker's control endpoints
//! (upload/apply/check/status/download) directly over TCP, the same way the
//! C `network_p2p.c` layer does. Verifies the broker correctly collapses the
//! S3 3-step flow before the full end-to-end two-node test runs.

use sync_p2p::protocol::{PutRequest, Request, Response, put, roundtrip};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn broker_serves_collapsed_s3_flow() {
    let broker = sync_p2p::Broker::start().await.unwrap();
    let addr = broker.address();
    let host_owned = addr.trim_start_matches("p2p://").to_string();
    let host = host_owned.as_str();
    let db = "test-db";
    let site = "site-1";

    // 1. upload (GET) → {"url":"mem://..."}
    let upload_ep = format!("{addr}/v2/cloudsync/databases/{db}/{site}/upload");
    let resp = roundtrip(
        host,
        &Request {
            token: String::new(),
            endpoint: upload_ep,
            is_post: false,
            body: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(resp.status, 200);
    let body = String::from_utf8(resp.body.unwrap().clone()).unwrap();
    let url: String = serde_json::from_str::<serde_json::Value>(&body)
        .unwrap()
        .get("url")
        .unwrap()
        .as_str()
        .unwrap()
        .to_string();
    assert!(
        url.starts_with("mem://"),
        "upload returns a mem:// URL, got {url}"
    );

    // 2. send_buffer (PUT) the blob to that mem:// URL.
    let blob = b"hello-sync-payload".to_vec();
    let put_resp = put(
        host,
        &PutRequest {
            token: String::new(),
            url: url.clone(),
            blob,
        },
    )
    .await
    .unwrap();
    assert!(put_resp.ok);

    // 3. apply (POST {"url":"...","dbVersionMin":1,"dbVersionMax":5})
    let apply_body = serde_json::json!({
        "url": url,
        "dbVersionMin": 1,
        "dbVersionMax": 5,
    })
    .to_string()
    .into_bytes();
    let apply_ep = format!("{addr}/v2/cloudsync/databases/{db}/{site}/apply");
    let resp = roundtrip(
        host,
        &Request {
            token: String::new(),
            endpoint: apply_ep,
            is_post: true,
            body: Some(apply_body),
        },
    )
    .await
    .unwrap();
    assert_eq!(resp.status, 200);
    let body = String::from_utf8(resp.body.unwrap().clone()).unwrap();
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v["lastOptimisticVersion"], 5);
    assert_eq!(v["gaps"], serde_json::json!([]));

    // 4. status (GET) → lastOptimisticVersion: 5
    let status_ep = format!("{addr}/v2/cloudsync/databases/{db}/{site}/status");
    let resp = roundtrip(
        host,
        &Request {
            token: String::new(),
            endpoint: status_ep,
            is_post: false,
            body: None,
        },
    )
    .await
    .unwrap();
    let v: serde_json::Value =
        serde_json::from_str(&String::from_utf8(resp.body.unwrap()).unwrap()).unwrap();
    assert_eq!(v["lastOptimisticVersion"], 5);

    // 5. check (POST {"dbVersion":0,"seq":0}) → {"url":"mem://..."} (a fresh copy)
    let check_body = serde_json::json!({ "dbVersion": 0, "seq": 0 })
        .to_string()
        .into_bytes();
    let check_ep = format!("{addr}/v2/cloudsync/databases/{db}/{site}/check");
    let resp = roundtrip(
        host,
        &Request {
            token: String::new(),
            endpoint: check_ep,
            is_post: true,
            body: Some(check_body),
        },
    )
    .await
    .unwrap();
    let v: serde_json::Value =
        serde_json::from_str(&String::from_utf8(resp.body.unwrap()).unwrap()).unwrap();
    let dl_url = v["url"].as_str().unwrap().to_string();
    assert!(
        dl_url.starts_with("mem://"),
        "check returns a download URL, got {dl_url}"
    );

    // 6. download (GET mem://url) → the raw blob bytes.
    let dl_resp = roundtrip(
        host,
        &Request {
            token: String::new(),
            endpoint: dl_url,
            is_post: false,
            body: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(dl_resp.status, 200);
    assert_eq!(dl_resp.body.unwrap(), b"hello-sync-payload");

    // 7. check again with dbVersion >= 5 → 204 No Content (nothing newer).
    //    The core's check_internal treats a non-BUFFER response as "no changes".
    let check_body = serde_json::json!({ "dbVersion": 5, "seq": 0 })
        .to_string()
        .into_bytes();
    let check_ep = format!("{addr}/v2/cloudsync/databases/{db}/{site}/check");
    let resp = roundtrip(
        host,
        &Request {
            token: String::new(),
            endpoint: check_ep,
            is_post: true,
            body: Some(check_body),
        },
    )
    .await
    .unwrap();
    assert_eq!(resp.status, 204, "nothing newer → 204 No Content");
    assert!(resp.body.is_none(), "204 has no body");

    broker.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn broker_rejects_bad_endpoint() {
    let broker = sync_p2p::Broker::start().await.unwrap();
    let host_owned = broker.address().trim_start_matches("p2p://").to_string();
    let host = host_owned.as_str();
    let resp: Response = roundtrip(
        host,
        &Request {
            token: String::new(),
            endpoint: "garbage://nope".into(),
            is_post: false,
            body: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(resp.status, 500);
    broker.stop().await;
}

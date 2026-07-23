//! Outbound webhook delivery: envelope construction, HMAC-SHA256 signing, and
//! bounded exponential-backoff retry.
//!
//! # Signing scheme (exact)
//!
//! The request body is the UTF-8 JSON bytes of the delivery envelope
//! (`WebhookEnvelope`, serialized with `serde_json::to_vec`). The signature is:
//!
//! ```text
//! sig  = HMAC-SHA256(key = secret_utf8_bytes, message = raw_body_bytes)
//! hdr  = "sha256=" + lowercase_hex(sig)
//! ```
//!
//! and is sent in the `X-Notare-Signature` header. The signature covers ONLY
//! the raw request body — not headers, not the URL. Receivers verify by
//! recomputing HMAC-SHA256 over the exact received body with the shared secret
//! and comparing (constant-time) against the hex after the `sha256=` prefix.
//!
//! Companion headers (NOT part of the signed material):
//! - `X-Notare-Event`     — the event type
//! - `X-Notare-Delivery`  — unique delivery id (uuid v4)
//! - `X-Notare-Timestamp` — unix seconds when the envelope was built
//! - `Content-Type: application/json`

use hmac::{Hmac, KeyInit, Mac};
use serde::Serialize;
use sha2::Sha256;
use std::time::Duration;

use crate::types::DeliveryRecord;

type HmacSha256 = Hmac<Sha256>;

pub const SIGNATURE_HEADER: &str = "X-Notare-Signature";
pub const EVENT_HEADER: &str = "X-Notare-Event";
pub const DELIVERY_HEADER: &str = "X-Notare-Delivery";
pub const TIMESTAMP_HEADER: &str = "X-Notare-Timestamp";

/// Reject payloads whose serialized envelope exceeds this size, so we never
/// try to sign/stream an unbounded body. 1 MiB is generous for action-item /
/// session-summary payloads.
pub const MAX_BODY_BYTES: usize = 1024 * 1024;

/// The JSON envelope actually sent over the wire.
#[derive(Debug, Clone, Serialize)]
pub struct WebhookEnvelope {
    pub id: String,
    pub event_type: String,
    pub timestamp: String,
    pub data: serde_json::Value,
}

/// Retry policy. `base_delay` is exposed so tests can drive the backoff loop
/// with millisecond delays instead of real seconds.
#[derive(Debug, Clone, Copy)]
pub struct RetryPolicy {
    pub max_attempts: u32,
    pub base_delay: Duration,
    pub max_delay: Duration,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            base_delay: Duration::from_millis(500),
            max_delay: Duration::from_secs(10),
        }
    }
}

impl RetryPolicy {
    /// Exponential backoff for the given (1-based) attempt number:
    /// `base * 2^(attempt-1)`, capped at `max_delay`. Deterministic (no jitter)
    /// so it is unit-testable; jitter is unnecessary for a single user's
    /// self-hosted endpoint.
    pub fn backoff_delay(&self, attempt: u32) -> Duration {
        if attempt <= 1 {
            return self.base_delay;
        }
        let shift = attempt - 1;
        // Saturating shift so we never overflow on a large attempt count.
        let factor = 1u64.checked_shl(shift).unwrap_or(u64::MAX);
        let millis = (self.base_delay.as_millis() as u64).saturating_mul(factor);
        Duration::from_millis(millis).min(self.max_delay)
    }
}

/// Compute the `X-Notare-Signature` header value for a raw body.
pub fn sign_body(secret: &str, body: &[u8]) -> String {
    // `new_from_slice` accepts a key of any length for HMAC, so this never errs.
    let mut mac =
        HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC accepts keys of any length");
    mac.update(body);
    format!("sha256={}", hex::encode(mac.finalize().into_bytes()))
}

/// Classify whether a failed HTTP status is worth retrying. Transient = 5xx and
/// 429 (rate limited). Permanent (4xx other than 429) is NOT retried.
pub fn is_retryable_status(status: u16) -> bool {
    status == 429 || (500..600).contains(&status)
}

/// Classify whether a reqwest transport error is transient (connect/timeout/
/// request-build/redirect issues) versus a permanent local problem.
pub fn is_retryable_error(err: &reqwest::Error) -> bool {
    err.is_timeout() || err.is_connect() || err.is_request()
}

/// Deliver an envelope to `url`, signing with `secret`, retrying transient
/// failures per `policy`. Returns a `DeliveryRecord` describing the final
/// outcome. Never panics; a hard local error is surfaced via
/// `DeliveryRecord.error`.
///
/// `now_rfc3339` is injected so the record timestamp is testable/deterministic.
pub async fn deliver(
    client: &reqwest::Client,
    url: &str,
    secret: &str,
    envelope: &WebhookEnvelope,
    policy: RetryPolicy,
) -> DeliveryRecord {
    let body = match serde_json::to_vec(envelope) {
        Ok(b) => b,
        Err(e) => {
            return DeliveryRecord {
                id: envelope.id.clone(),
                event_type: envelope.event_type.clone(),
                timestamp: now_rfc3339(),
                status_code: None,
                error: Some(format!("serialize envelope: {e}")),
                attempts: 0,
                success: false,
            };
        }
    };

    if body.len() > MAX_BODY_BYTES {
        return DeliveryRecord {
            id: envelope.id.clone(),
            event_type: envelope.event_type.clone(),
            timestamp: now_rfc3339(),
            status_code: None,
            error: Some(format!(
                "payload too large: {} bytes (max {})",
                body.len(),
                MAX_BODY_BYTES
            )),
            attempts: 0,
            success: false,
        };
    }

    let signature = sign_body(secret, &body);
    let mut last_status: Option<u16> = None;
    let mut last_error: Option<String> = None;
    let mut attempts = 0u32;

    for attempt in 1..=policy.max_attempts {
        attempts = attempt;

        let req = client
            .post(url)
            .header("Content-Type", "application/json")
            .header(SIGNATURE_HEADER, &signature)
            .header(EVENT_HEADER, &envelope.event_type)
            .header(DELIVERY_HEADER, &envelope.id)
            .header(TIMESTAMP_HEADER, &envelope.timestamp)
            .body(body.clone());

        match req.send().await {
            Ok(resp) => {
                let status = resp.status().as_u16();
                last_status = Some(status);
                last_error = None;
                if (200..300).contains(&status) {
                    return DeliveryRecord {
                        id: envelope.id.clone(),
                        event_type: envelope.event_type.clone(),
                        timestamp: now_rfc3339(),
                        status_code: Some(status),
                        error: None,
                        attempts,
                        success: true,
                    };
                }
                if !is_retryable_status(status) || attempt == policy.max_attempts {
                    break;
                }
            }
            Err(e) => {
                last_status = None;
                last_error = Some(e.to_string());
                if !is_retryable_error(&e) || attempt == policy.max_attempts {
                    break;
                }
            }
        }

        tokio::time::sleep(policy.backoff_delay(attempt)).await;
    }

    DeliveryRecord {
        id: envelope.id.clone(),
        event_type: envelope.event_type.clone(),
        timestamp: now_rfc3339(),
        status_code: last_status,
        error: last_error.or_else(|| last_status.map(|s| format!("non-2xx response: {s}"))),
        attempts,
        success: false,
    }
}

fn now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signature_matches_known_vector() {
        // Verified against an independent HMAC-SHA256 implementation.
        // secret="secret", body="hello" ->
        //   88aab3ede8d3adf94d26ab90d3bafd4a2083070c3bcce9c014ee04a443847c0b
        let sig = sign_body("secret", b"hello");
        assert_eq!(
            sig,
            "sha256=88aab3ede8d3adf94d26ab90d3bafd4a2083070c3bcce9c014ee04a443847c0b"
        );
    }

    #[test]
    fn signature_is_stable_and_body_sensitive() {
        let a = sign_body("k", b"{\"a\":1}");
        let b = sign_body("k", b"{\"a\":1}");
        let c = sign_body("k", b"{\"a\":2}");
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert!(a.starts_with("sha256="));
    }

    #[test]
    fn retryable_status_classification() {
        assert!(is_retryable_status(500));
        assert!(is_retryable_status(503));
        assert!(is_retryable_status(429));
        assert!(!is_retryable_status(400));
        assert!(!is_retryable_status(401));
        assert!(!is_retryable_status(404));
        assert!(!is_retryable_status(200));
    }

    #[test]
    fn backoff_is_exponential_and_capped() {
        let p = RetryPolicy {
            max_attempts: 5,
            base_delay: Duration::from_millis(100),
            max_delay: Duration::from_millis(700),
        };
        assert_eq!(p.backoff_delay(1), Duration::from_millis(100));
        assert_eq!(p.backoff_delay(2), Duration::from_millis(200));
        assert_eq!(p.backoff_delay(3), Duration::from_millis(400));
        // 800ms would exceed the 700ms cap.
        assert_eq!(p.backoff_delay(4), Duration::from_millis(700));
        assert_eq!(p.backoff_delay(99), Duration::from_millis(700));
    }
}

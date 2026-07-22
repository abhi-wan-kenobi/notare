use std::convert::Infallible;

use axum::{
    Json,
    extract::ws::{Message, WebSocket},
    http::StatusCode,
    response::{
        IntoResponse, Response,
        sse::{Event, Sse},
    },
};
use futures_util::{SinkExt, stream::SplitSink};
use owhisper_interface::batch_sse::{BatchSseMessage, EVENT_NAME};
use owhisper_interface::stream::StreamResponse;
use tokio::sync::mpsc;

pub type WsSender = SplitSink<WebSocket, Message>;

/// A single WS write must never block forever. A half-open socket — e.g. a
/// Tailscale path hiccup / DERP flap mid-stream — can leave `sender.send().await`
/// pending indefinitely, which silently freezes the entire session `select!`
/// loop (the client hangs at ~88% with no error). Bounding each write surfaces a
/// stuck connection as a normal send-failure instead of an infinite hang. The
/// window sits well above the 15s keepalive so a healthy-but-slow write is never
/// tripped.
pub const WS_SEND_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(20);

pub async fn send_ws(sender: &mut WsSender, value: &StreamResponse) -> bool {
    let payload = match serde_json::to_string(value) {
        Ok(payload) => payload,
        Err(error) => {
            tracing::warn!("failed to serialize ws response: {error}");
            return false;
        }
    };

    match tokio::time::timeout(WS_SEND_TIMEOUT, sender.send(Message::Text(payload.into()))).await {
        Ok(result) => result.is_ok(),
        Err(_) => {
            tracing::warn!(
                timeout_secs = WS_SEND_TIMEOUT.as_secs(),
                "ws_send_timed_out: write blocked past the timeout; treating the connection as broken"
            );
            false
        }
    }
}

pub async fn send_ws_best_effort(sender: &mut WsSender, value: &StreamResponse) {
    let payload = match serde_json::to_string(value) {
        Ok(payload) => payload,
        Err(error) => {
            tracing::warn!("failed to serialize ws response: {error}");
            return;
        }
    };

    if tokio::time::timeout(WS_SEND_TIMEOUT, sender.send(Message::Text(payload.into())))
        .await
        .is_err()
    {
        tracing::warn!(
            timeout_secs = WS_SEND_TIMEOUT.as_secs(),
            "ws_send_best_effort_timed_out: write blocked past the timeout"
        );
    }
}

pub fn json_error_response(status: StatusCode, error: &str, detail: impl Into<String>) -> Response {
    (
        status,
        Json(serde_json::json!({
            "error": error,
            "detail": detail.into(),
        })),
    )
        .into_response()
}

pub fn batch_sse_response(event_rx: mpsc::UnboundedReceiver<BatchSseMessage>) -> Response {
    let events_stream = futures_util::stream::unfold(event_rx, |mut rx| async move {
        rx.recv().await.map(|message| {
            let event = match Event::default().event(EVENT_NAME).json_data(&message) {
                Ok(event) => event,
                Err(error) => {
                    tracing::warn!("failed to serialize batch SSE event: {error}");
                    Event::default()
                        .event(EVENT_NAME)
                        .data(r#"{"error":"transcription_failed","detail":"failed to serialize SSE event"}"#)
                }
            };
            (Ok::<_, Infallible>(event), rx)
        })
    });

    Sse::new(events_stream).into_response()
}

pub fn format_timestamp_now() -> String {
    let duration = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let total_secs = duration.as_secs();
    let millis = duration.subsec_millis();

    let mut days = total_secs / 86_400;
    let day_secs = (total_secs % 86_400) as u32;
    let hours = day_secs / 3_600;
    let minutes = (day_secs % 3_600) / 60;
    let seconds = day_secs % 60;

    let mut year = 1970i32;
    loop {
        let year_days = if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) {
            366
        } else {
            365
        };
        if days < year_days {
            break;
        }
        days -= year_days;
        year += 1;
    }

    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let month_days = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut month = 1u32;
    for month_len in month_days {
        if days < month_len {
            break;
        }
        days -= month_len;
        month += 1;
    }

    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z",
        year,
        month,
        days + 1,
        hours,
        minutes,
        seconds,
        millis
    )
}

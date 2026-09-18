//! Protocol v2 — the wire format between two notare sync peers.
//!
//! The 0.7 engine swap (cr-sqlite) retires the old CloudSync broker flow
//! (upload/apply/check object-store round trips over HTTP-shaped
//! `Request`/`Response` frames). Protocol v2 speaks sync **directly**: a
//! pull-only, symmetric session between two allowlisted peers over one iroh
//! bi-stream, whole-frame encrypted with [`crate::crypto::encrypt`].
//!
//! ## Framing
//!
//! Unchanged from v1 at the byte level: 4-byte big-endian length prefix, then
//! `len` bytes of JSON, hard-capped at [`MAX_FRAME_BYTES`] (64 MiB). The JSON
//! shape is wire-incompatible with v1 (the ALPN bump to
//! [`crate::agent::SYNC_ALPN`] makes old and new peers refuse each other
//! cleanly at the TLS layer rather than misparse frames).
//!
//! ## Session (pull-only, symmetric)
//!
//! Each device is *client* toward every allowlisted peer each tick, and
//! *server* for accepted streams — same protocol, same frames, mirrored
//! roles. Client: [`SyncMessage::Hello`] → [`SyncMessage::Pull`] with the
//! cursor persisted for that peer → apply each
//! [`SyncMessage::Changes`] page → [`SyncMessage::Ack`] when `more == false`.
//! There is no push; both sides pulling makes idempotency trivial (cr-sqlite
//! apply is a no-op for a losing `(col_version, site_id)`) and removes the
//! broker's ordering log entirely. Three-node transitivity is free: B's
//! `crsql_changes` carries C's rows under C's `site_id`.
//!
//! ## Backpressure
//!
//! The client controls pacing: one page in flight, next `Pull` starting at
//! the previous page's `next` cursor and capped at [`DEFAULT_MAX_BYTES`] (4
//! MiB, always at least one row), so the receiver's apply speed throttles the
//! sender. The 64 MiB frame cap stays as the hard guard.

use std::io;

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::source::{Change, ChangesPage, Cursor};

/// Hard cap on one wire frame: 4-byte BE length prefix, 64 MiB max.
pub const MAX_FRAME_BYTES: usize = 64 * 1024 * 1024;

/// Default page budget for a `Pull`, in serialized-change bytes (4 MiB).
pub const DEFAULT_MAX_BYTES: usize = 4 * 1024 * 1024;

/// One protocol v2 message. See the module docs for the session shape.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SyncMessage {
    /// Session opener from the client. `schema_version` is the peer's highest
    /// applied migration; a mismatch is answered with `Error` so a peer on an
    /// older schema is never handed a `cid` it lacks.
    Hello {
        site_id: Vec<u8>,
        db_version: i64,
        schema_version: i64,
    },
    /// Request the next page of changes after `after` (the cursor persisted
    /// for the serving peer), excluding `exclude_site`'s own changes, with a
    /// byte budget.
    Pull {
        after: Cursor,
        exclude_site: Vec<u8>,
        max_bytes: usize,
    },
    /// One page of changes. `more == false` means the session can move on
    /// (`Ack`); the client's next `Pull` starts at `next`.
    Changes { page: ChangesPage },
    /// Client confirmation that it applied through `applied_through` — the
    /// server records this as the `served` cursor for that peer.
    Ack { applied_through: Cursor },
    /// Terminal error from either side.
    Error { message: String },
}

/// Write one length-prefixed JSON frame.
pub async fn write_frame<W: AsyncWriteExt + Unpin, T: Serialize>(
    w: &mut W,
    value: &T,
) -> io::Result<()> {
    let json = serde_json::to_vec(value)?;
    let len = (json.len() as u32).to_be_bytes();
    w.write_all(&len).await?;
    w.write_all(&json).await?;
    w.flush().await?;
    Ok(())
}

/// Read one length-prefixed JSON frame.
pub async fn read_frame<R: AsyncReadExt + Unpin, T: for<'de> Deserialize<'de>>(
    r: &mut R,
) -> io::Result<T> {
    let mut len_buf = [0u8; 4];
    r.read_exact(&mut len_buf).await?;
    let len = u32::from_be_bytes(len_buf) as usize;
    if len > MAX_FRAME_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("frame too large: {len} bytes"),
        ));
    }
    let mut buf = vec![0u8; len];
    r.read_exact(&mut buf).await?;
    serde_json::from_slice(&buf).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hello() -> SyncMessage {
        SyncMessage::Hello {
            site_id: vec![1, 2, 3, 4],
            db_version: 7,
            schema_version: 20260907,
        }
    }

    /// A frame must round-trip through the real framing helpers and must not
    /// exceed the 64 MiB cap.
    #[tokio::test]
    async fn frames_roundtrip() {
        let mut buf = Vec::new();
        write_frame(&mut buf, &hello()).await.unwrap();
        // One frame: 4-byte prefix + payload, nothing more.
        let len = u32::from_be_bytes(buf[..4].try_into().unwrap()) as usize;
        assert_eq!(buf.len(), 4 + len);

        let back: SyncMessage = read_frame(&mut buf.as_slice()).await.unwrap();
        assert_eq!(back, hello());
    }

    /// The wire format is JSON with a `type` discriminator — pinned so an
    /// accidental serde reshuffle (e.g. dropping the tag) cannot silently
    /// change the protocol.
    #[test]
    fn message_tag_is_snake_case_json() {
        let json = serde_json::to_value(hello()).unwrap();
        assert_eq!(json["type"], "hello");
        assert_eq!(json["site_id"], serde_json::json!([1, 2, 3, 4]));

        let pull = SyncMessage::Pull {
            after: Cursor {
                db_version: 5,
                seq: 0,
            },
            exclude_site: vec![9, 9],
            max_bytes: DEFAULT_MAX_BYTES,
        };
        let json = serde_json::to_value(&pull).unwrap();
        assert_eq!(json["type"], "pull");
        assert_eq!(json["after"]["db_version"], 5);
    }

    /// Frames over the cap are refused by the reader, never buffered.
    #[tokio::test]
    async fn oversized_frame_is_refused() {
        let mut buf: &[u8] = &(MAX_FRAME_BYTES as u32 + 1).to_be_bytes();
        let err = read_frame::<_, SyncMessage>(&mut buf).await.unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
        assert!(err.to_string().contains("frame too large"));
    }

    /// A garbage payload must fail deserialization, not panic or hang.
    #[tokio::test]
    async fn garbage_frame_fails_cleanly() {
        let payload = b"not json at all";
        let mut buf = Vec::with_capacity(4 + payload.len());
        buf.extend_from_slice(&(payload.len() as u32).to_be_bytes());
        buf.extend_from_slice(payload);
        let mut reader = buf.as_slice();
        assert!(read_frame::<_, SyncMessage>(&mut reader).await.is_err());
    }
}

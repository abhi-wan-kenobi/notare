//! Framed length-prefixed TCP protocol between a cloudsync site (the C FFI
//! bridge) and the in-process/cross-process [`crate::broker::Broker`] server.
//!
//! Every frame is: 4-byte big-endian length, then `len` bytes of JSON.
//! Blob bodies (upload PUT, check/download GET) are base64 inside the JSON so
//! the whole frame stays a single JSON object — keeping the protocol a pair of
//! plain `serde_json::Value` round-trips and avoiding a second framing channel.
//!
//! This is a SPIKE transport: localhost, unencrypted, no auth. Production
//! (iroh/QUIC, encryption, relay, pairing) is out of scope — see the S1 report.

use std::io;

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

/// One request from a site (the C FFI bridge) to the broker.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request {
    /// The full endpoint URL the cloudsync core handed us, e.g.
    /// `p2p://127.0.0.1:38321/<dbId>/<siteId>/upload`.
    pub endpoint: String,
    /// `is_post_request` from `network_receive_buffer`; false ⇒ GET semantics.
    pub is_post: bool,
    /// POST body (the JSON payload), base64 of the raw bytes, or null for GET.
    #[serde(
        default,
        with = "serde_opt_bytes_base64",
        skip_serializing_if = "Option::is_none"
    )]
    pub body: Option<Vec<u8>>,
}

/// One response from the broker back to the site.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    /// HTTP-ish status: 200 = OK with a body, 204 = OK no body, 4xx/5xx = error.
    pub status: u16,
    /// Response body bytes (base64). For `receive`, this is the JSON the core
    /// parses (e.g. `{"url":"mem://..."}` or `{"lastOptimisticVersion":...}`).
    /// `None` is serialized as `"body":null` (not skipped) so the C side's
    /// manual `"body"` lookup always finds the key.
    #[serde(default, with = "serde_opt_bytes_base64")]
    pub body: Option<Vec<u8>>,
    /// Diagnostic message on error.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// `send_buffer` (PUT) is a separate frame shape: raw blob upload to a `mem://`
/// URL the broker handed back from the upload step.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PutRequest {
    /// The `mem://<id>` URL returned by the broker's upload endpoint.
    pub url: String,
    #[serde(with = "serde_bytes_base64")]
    pub blob: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PutResponse {
    pub ok: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

mod serde_bytes_base64 {
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S: Serializer>(bytes: &Vec<u8>, s: S) -> Result<S::Ok, S::Error> {
        STANDARD.encode(bytes).serialize(s)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<u8>, D::Error> {
        let s = String::deserialize(d)?;
        STANDARD.decode(s).map_err(serde::de::Error::custom)
    }
}

/// serde helper for `Option<Vec<u8>>` base64 fields.
mod serde_opt_bytes_base64 {
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S: Serializer>(bytes: &Option<Vec<u8>>, s: S) -> Result<S::Ok, S::Error> {
        match bytes {
            Some(b) => STANDARD.encode(b).serialize(s),
            None => s.serialize_none(),
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Vec<u8>>, D::Error> {
        let opt: Option<String> = Option::deserialize(d)?;
        match opt {
            Some(s) => STANDARD
                .decode(s)
                .map(Some)
                .map_err(serde::de::Error::custom),
            None => Ok(None),
        }
    }
}

/// Write a length-prefixed JSON frame.
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

/// Read a length-prefixed JSON frame.
pub async fn read_frame<R: AsyncReadExt + Unpin, T: for<'de> Deserialize<'de>>(
    r: &mut R,
) -> io::Result<T> {
    let mut len_buf = [0u8; 4];
    r.read_exact(&mut len_buf).await?;
    let len = u32::from_be_bytes(len_buf) as usize;
    if len > 64 * 1024 * 1024 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("frame too large: {len} bytes"),
        ));
    }
    let mut buf = vec![0u8; len];
    r.read_exact(&mut buf).await?;
    serde_json::from_slice(&buf).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

/// Convenience: one-shot request/response over a fresh TCP connection.
pub async fn roundtrip(addr: &str, req: &Request) -> io::Result<Response> {
    let mut stream = TcpStream::connect(addr).await?;
    write_frame(&mut stream, req).await?;
    let resp = read_frame::<_, Response>(&mut stream).await?;
    Ok(resp)
}

/// One-shot PUT (send_buffer) over a fresh TCP connection.
pub async fn put(addr: &str, req: &PutRequest) -> io::Result<PutResponse> {
    let mut stream = TcpStream::connect(addr).await?;
    write_frame(&mut stream, req).await?;
    let resp = read_frame::<_, PutResponse>(&mut stream).await?;
    Ok(resp)
}

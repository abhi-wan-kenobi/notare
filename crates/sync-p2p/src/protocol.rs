//! Sync protocol v2 — the wire format spoken on ALPN [`SYNC_ALPN`] over one
//! iroh bi-stream per session.
//!
//! ## Messages
//!
//! Both peers send a [`Message::Hello`] first. After that each side *pulls*
//! from the other: `Pull{after, limit}` is answered with `Changes(page)`; the
//! puller applies the page, persists the cursor, and sends `Ack{cursor}`. A
//! page with `more == false` followed by its `Ack` ends that direction.
//! `Error{code, msg}` aborts the session. Both directions run concurrently on
//! the one stream; see [`crate::session`].
//!
//! ## Framing and sealing
//!
//! Every message is one frame: a 4-byte big-endian length, then the sealed
//! payload (`[24-byte nonce || XChaCha20-Poly1305 ciphertext+tag]`, see
//! [`crate::crypto`]). The length is capped at [`MAX_FRAME_LEN`] (64 MiB)
//! *before* anything is allocated, so a peer cannot make us buffer more.
//!
//! The sealed plaintext is `[role: u8][seq: u64 BE][postcard(Message)]`.
//! QUIC/TLS already authenticates the stream, but the per-pair key is the same
//! in both directions, so without these two fields a peer could reflect one of
//! our own frames back at us, and nothing at this layer would notice a replayed
//! or dropped frame. `role` is the sender's role (dialer/acceptor) and `seq` a
//! per-direction counter starting at 0; the reader rejects a wrong role or a
//! non-consecutive `seq`.
//!
//! ## Why postcard
//!
//! Change rows carry `Vec<u8>` blobs (primary keys, values, site ids). JSON
//! encodes those as arrays of numbers (~3-4 bytes per byte); postcard writes
//! them raw with varint lengths. It is also schema-less on the wire, matching
//! the plain `serde` derives in [`crate::change`].

use std::io;

use iroh::{PublicKey, SecretKey};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::change::{WireCursor, WirePage};
use crate::crypto::{self, CryptoError};

/// The ALPN the sync transport speaks over iroh. Both peers must match.
pub const SYNC_ALPN: &[u8] = b"/notare/sync/2";

/// Protocol version carried in [`Message::Hello`].
pub const PROTO_VERSION: u32 = 2;

/// Largest frame (sealed payload) either side will write or accept.
pub const MAX_FRAME_LEN: usize = 64 * 1024 * 1024;

/// Machine-readable reason in [`Message::Error`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ErrorCode {
    /// The peer sent something the protocol does not allow here.
    BadRequest,
    /// Hello protocol version or identity is not acceptable.
    Unsupported,
    /// The sender failed locally (database error etc.). `msg` is generic.
    Internal,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Message {
    Hello {
        /// z-base-32 device id; must equal the TLS-authenticated remote id.
        device_id: String,
        /// The sender's cr-sqlite site id.
        site_id: Vec<u8>,
        proto: u32,
    },
    /// Ask for changes after `after`, at most `limit` rows.
    Pull {
        after: WireCursor,
        limit: u32,
    },
    Changes(WirePage),
    /// "I applied everything up to `cursor`."
    Ack {
        cursor: WireCursor,
    },
    Error {
        code: ErrorCode,
        msg: String,
    },
}

/// Which end of the stream a frame was sealed by.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// Opened the connection (`sync_with`).
    Dialer = 0,
    /// Accepted it.
    Acceptor = 1,
}

impl Role {
    pub fn peer(self) -> Role {
        match self {
            Role::Dialer => Role::Acceptor,
            Role::Acceptor => Role::Dialer,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ProtocolError {
    #[error("io: {0}")]
    Io(#[from] io::Error),
    #[error("frame of {0} bytes exceeds the {MAX_FRAME_LEN}-byte cap")]
    FrameTooLarge(usize),
    #[error("frame failed authentication: {0}")]
    Crypto(#[from] CryptoError),
    #[error("malformed message: {0}")]
    Decode(String),
    #[error("frame sealed by the wrong role (reflected frame?)")]
    WrongRole,
    #[error("frame out of sequence: expected {expected}, got {got}")]
    OutOfSequence { expected: u64, got: u64 },
}

const HEADER_LEN: usize = 1 + 8;

/// Seals and writes frames for one direction of a stream.
pub struct FrameWriter<W> {
    w: W,
    secret: SecretKey,
    peer: PublicKey,
    role: Role,
    seq: u64,
}

impl<W: AsyncWrite + Unpin> FrameWriter<W> {
    pub fn new(w: W, secret: SecretKey, peer: PublicKey, role: Role) -> Self {
        Self {
            w,
            secret,
            peer,
            role,
            seq: 0,
        }
    }

    /// Seal and write one message. Returns [`ProtocolError::FrameTooLarge`]
    /// *without writing anything or consuming a sequence number* when the sealed
    /// frame would exceed [`MAX_FRAME_LEN`], so the caller can retry smaller.
    pub async fn send(&mut self, msg: &Message) -> Result<(), ProtocolError> {
        let body = postcard::to_stdvec(msg).map_err(|e| ProtocolError::Decode(e.to_string()))?;
        let mut plain = Vec::with_capacity(HEADER_LEN + body.len());
        plain.push(self.role as u8);
        plain.extend_from_slice(&self.seq.to_be_bytes());
        plain.extend_from_slice(&body);
        let sealed = crypto::encrypt(&self.secret, &self.peer, &plain)?;
        if sealed.len() > MAX_FRAME_LEN {
            return Err(ProtocolError::FrameTooLarge(sealed.len()));
        }
        self.w
            .write_all(&(sealed.len() as u32).to_be_bytes())
            .await?;
        self.w.write_all(&sealed).await?;
        self.w.flush().await?;
        self.seq += 1;
        Ok(())
    }

    /// The underlying writer (tests use it to forge frames on a live stream).
    pub fn get_mut(&mut self) -> &mut W {
        &mut self.w
    }

    /// Signal end-of-stream (QUIC `finish`) to the peer.
    pub async fn shutdown(&mut self) -> io::Result<()> {
        self.w.shutdown().await
    }
}

/// Reads and opens frames for one direction of a stream.
pub struct FrameReader<R> {
    r: R,
    secret: SecretKey,
    peer: PublicKey,
    /// Role the *sender* must have sealed with.
    expect: Role,
    seq: u64,
}

impl<R: AsyncRead + Unpin> FrameReader<R> {
    /// `local_role` is our own role; frames must carry the opposite one.
    pub fn new(r: R, secret: SecretKey, peer: PublicKey, local_role: Role) -> Self {
        Self {
            r,
            secret,
            peer,
            expect: local_role.peer(),
            seq: 0,
        }
    }

    /// Next message, or `Ok(None)` on a clean end-of-stream at a frame boundary.
    pub async fn recv(&mut self) -> Result<Option<Message>, ProtocolError> {
        let mut len_buf = [0u8; 4];
        match self.r.read_exact(&mut len_buf).await {
            Ok(_) => {}
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => {
                return Ok(None);
            }
            Err(e) => return Err(e.into()),
        }
        let len = u32::from_be_bytes(len_buf) as usize;
        if len > MAX_FRAME_LEN {
            return Err(ProtocolError::FrameTooLarge(len));
        }
        let mut sealed = vec![0u8; len];
        self.r.read_exact(&mut sealed).await?;
        let plain = crypto::decrypt(&self.secret, &self.peer, &sealed)?;
        if plain.len() < HEADER_LEN {
            return Err(ProtocolError::Decode("short frame".into()));
        }
        if plain[0] != self.expect as u8 {
            return Err(ProtocolError::WrongRole);
        }
        let got = u64::from_be_bytes(plain[1..HEADER_LEN].try_into().expect("8 bytes"));
        if got != self.seq {
            return Err(ProtocolError::OutOfSequence {
                expected: self.seq,
                got,
            });
        }
        let msg = postcard::from_bytes(&plain[HEADER_LEN..])
            .map_err(|e| ProtocolError::Decode(e.to_string()))?;
        self.seq += 1;
        Ok(Some(msg))
    }
}

/// Largest pairing frame. Pairing messages are a few hundred bytes; the cap
/// keeps an unauthenticated joiner from making us buffer more.
pub const MAX_PAIRING_FRAME_LEN: usize = 64 * 1024;

/// The offerer's answer to a [`crate::pairing::JoinMsg`] on
/// [`crate::pairing::PAIRING_ALPN`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PairingReply {
    Accept(crate::pairing::AcceptMsg),
    Reject(crate::pairing::RejectMsg),
}

/// Write one pairing frame: 4-byte big-endian length, then JSON. Pairing runs
/// before the peers trust each other, so there is no per-pair key to seal with;
/// the join proof (HMAC over the offer secret) is what authenticates it.
pub async fn write_json_frame<W: AsyncWrite + Unpin, T: Serialize>(
    w: &mut W,
    value: &T,
) -> io::Result<()> {
    let json = serde_json::to_vec(value)?;
    if json.len() > MAX_PAIRING_FRAME_LEN {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "pairing frame too large",
        ));
    }
    w.write_all(&(json.len() as u32).to_be_bytes()).await?;
    w.write_all(&json).await?;
    w.flush().await
}

/// Read one pairing frame written by [`write_json_frame`].
pub async fn read_json_frame<R: AsyncRead + Unpin, T: serde::de::DeserializeOwned>(
    r: &mut R,
) -> io::Result<T> {
    let mut len_buf = [0u8; 4];
    r.read_exact(&mut len_buf).await?;
    let len = u32::from_be_bytes(len_buf) as usize;
    if len > MAX_PAIRING_FRAME_LEN {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "pairing frame too large",
        ));
    }
    let mut buf = vec![0u8; len];
    r.read_exact(&mut buf).await?;
    serde_json::from_slice(&buf).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::change::{WireChange, WireValue};

    fn pair() -> (SecretKey, SecretKey) {
        (SecretKey::generate(), SecretKey::generate())
    }

    fn page() -> WirePage {
        let change = |i: i64, val: WireValue| WireChange {
            table: "notes".into(),
            pk: WireValue::Blob(vec![1, 2, 3, i as u8]),
            cid: "body".into(),
            val,
            col_version: i,
            db_version: 10 + i,
            site_id: vec![9; 16],
            cl: 1,
            seq: i,
        };
        WirePage {
            changes: vec![
                change(0, WireValue::Null),
                change(1, WireValue::Integer(i64::MIN)),
                change(2, WireValue::Real(-0.5)),
                change(3, WireValue::Text("héllo".into())),
                change(4, WireValue::Blob(vec![0, 255, 7])),
            ],
            next: WireCursor {
                db_version: 14,
                seq: 4,
            },
            more: true,
        }
    }

    /// Every message shape, including all `WireValue` variants (adjacently
    /// tagged, which non-self-describing formats can get wrong), survives a
    /// seal/open round trip in order.
    #[tokio::test]
    async fn round_trips_every_message_shape() {
        let (sk_a, sk_b) = pair();
        let (a, b) = tokio::io::duplex(1 << 20);
        let mut w = FrameWriter::new(a, sk_a.clone(), sk_b.public(), Role::Dialer);
        let mut r = FrameReader::new(b, sk_b, sk_a.public(), Role::Acceptor);

        let msgs = vec![
            Message::Hello {
                device_id: "abc".into(),
                site_id: vec![1; 16],
                proto: PROTO_VERSION,
            },
            Message::Pull {
                after: WireCursor {
                    db_version: 3,
                    seq: 1,
                },
                limit: 500,
            },
            Message::Changes(page()),
            Message::Ack {
                cursor: WireCursor::default(),
            },
            Message::Error {
                code: ErrorCode::Internal,
                msg: "boom".into(),
            },
        ];
        for m in &msgs {
            w.send(m).await.unwrap();
        }
        w.shutdown().await.unwrap();
        for m in &msgs {
            assert_eq!(&r.recv().await.unwrap().unwrap(), m);
        }
        assert!(r.recv().await.unwrap().is_none(), "clean EOF at a boundary");
    }

    async fn sealed_frame(
        secret: &SecretKey,
        peer: &PublicKey,
        role: Role,
        seq: u64,
        msg: &Message,
    ) -> Vec<u8> {
        let mut plain = vec![role as u8];
        plain.extend_from_slice(&seq.to_be_bytes());
        plain.extend_from_slice(&postcard::to_stdvec(msg).unwrap());
        let sealed = crypto::encrypt(secret, peer, &plain).unwrap();
        let mut out = (sealed.len() as u32).to_be_bytes().to_vec();
        out.extend_from_slice(&sealed);
        out
    }

    fn pull() -> Message {
        Message::Pull {
            after: WireCursor::default(),
            limit: 1,
        }
    }

    #[tokio::test]
    async fn flipped_ciphertext_bit_is_rejected() {
        let (sk_a, sk_b) = pair();
        let mut frame = sealed_frame(&sk_a, &sk_b.public(), Role::Dialer, 0, &pull()).await;
        let last = frame.len() - 1;
        frame[last] ^= 1;
        let mut r = FrameReader::new(&frame[..], sk_b, sk_a.public(), Role::Acceptor);
        assert!(matches!(r.recv().await, Err(ProtocolError::Crypto(_))));
    }

    #[tokio::test]
    async fn frame_sealed_for_another_device_is_rejected() {
        let (sk_a, sk_b) = pair();
        let sk_c = SecretKey::generate();
        // Sealed A->C, fed to B (which believes the peer is A).
        let frame = sealed_frame(&sk_a, &sk_c.public(), Role::Dialer, 0, &pull()).await;
        let mut r = FrameReader::new(&frame[..], sk_b, sk_a.public(), Role::Acceptor);
        assert!(matches!(r.recv().await, Err(ProtocolError::Crypto(_))));
    }

    #[tokio::test]
    async fn own_frame_reflected_back_is_rejected() {
        let (sk_a, sk_b) = pair();
        // A (dialer) sealed this; reflecting it to A must fail on role even
        // though A and B share the same pair key.
        let frame = sealed_frame(&sk_a, &sk_b.public(), Role::Dialer, 0, &pull()).await;
        let mut r = FrameReader::new(&frame[..], sk_a, sk_b.public(), Role::Dialer);
        assert!(matches!(r.recv().await, Err(ProtocolError::WrongRole)));
    }

    #[tokio::test]
    async fn replayed_dropped_or_reordered_frames_are_rejected() {
        let (sk_a, sk_b) = pair();
        let f0 = sealed_frame(&sk_a, &sk_b.public(), Role::Dialer, 0, &pull()).await;
        let f1 = sealed_frame(&sk_a, &sk_b.public(), Role::Dialer, 1, &pull()).await;

        let replay = [f0.clone(), f0.clone()].concat();
        let mut r = FrameReader::new(&replay[..], sk_b.clone(), sk_a.public(), Role::Acceptor);
        r.recv().await.unwrap().unwrap();
        assert!(matches!(
            r.recv().await,
            Err(ProtocolError::OutOfSequence {
                expected: 1,
                got: 0
            })
        ));

        let dropped = f1.clone();
        let mut r = FrameReader::new(&dropped[..], sk_b, sk_a.public(), Role::Acceptor);
        assert!(matches!(
            r.recv().await,
            Err(ProtocolError::OutOfSequence {
                expected: 0,
                got: 1
            })
        ));
    }

    #[tokio::test]
    async fn oversize_length_prefix_is_refused_before_allocating() {
        let (sk_a, sk_b) = pair();
        let bytes = ((MAX_FRAME_LEN + 1) as u32).to_be_bytes();
        let mut r = FrameReader::new(&bytes[..], sk_b, sk_a.public(), Role::Acceptor);
        assert!(matches!(
            r.recv().await,
            Err(ProtocolError::FrameTooLarge(_))
        ));
    }

    #[tokio::test]
    async fn truncated_frame_is_an_error_not_a_clean_eof() {
        let (sk_a, sk_b) = pair();
        let frame = sealed_frame(&sk_a, &sk_b.public(), Role::Dialer, 0, &pull()).await;
        let cut = &frame[..frame.len() - 3];
        let mut r = FrameReader::new(cut, sk_b, sk_a.public(), Role::Acceptor);
        assert!(matches!(r.recv().await, Err(ProtocolError::Io(_))));
    }

    #[tokio::test]
    async fn oversized_message_is_reported_without_writing_or_burning_a_seq() {
        let (sk_a, sk_b) = pair();
        let mut sink = Vec::new();
        let mut w = FrameWriter::new(&mut sink, sk_a, sk_b.public(), Role::Dialer);
        let huge = Message::Changes(WirePage {
            changes: vec![WireChange {
                table: "t".into(),
                pk: WireValue::Null,
                cid: "c".into(),
                val: WireValue::Blob(vec![0; MAX_FRAME_LEN]),
                col_version: 1,
                db_version: 1,
                site_id: vec![],
                cl: 1,
                seq: 0,
            }],
            next: WireCursor::default(),
            more: false,
        });
        assert!(matches!(
            w.send(&huge).await,
            Err(ProtocolError::FrameTooLarge(_))
        ));
        assert_eq!(w.seq, 0);
        drop(w);
        assert!(sink.is_empty());
    }
}

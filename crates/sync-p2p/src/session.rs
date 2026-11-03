//! One sync session with one connected peer, both directions at once.
//!
//! A session runs over a single bi-stream (anything `AsyncRead + AsyncWrite`;
//! iroh streams in production, `tokio::io::duplex` in tests). After the
//! [`Message::Hello`] exchange each side
//!
//! * **pulls**: asks the peer for changes after `source.peer_cursor(peer)`,
//!   page by page while `more`, applying each page and persisting the cursor
//!   (`apply` first, `set_peer_cursor` second, so a crash in between replays a
//!   page — `apply` is idempotent — and never skips one);
//! * **serves**: answers the peer's `Pull`s from
//!   `source.changes_since(after, exclude_site = peer's site id, limit)`.
//!
//! Direction ends are signalled in-band: the last page of a pull has
//! `more == false`, and its `Ack` tells the server that direction is complete.
//! A side is done when its own pull completed *and* the peer acked the last
//! page it served; it then finishes its send half and waits for the peer's
//! EOF, so neither side drops the connection with data still in flight.
//!
//! A dedicated task drains the receive half into a small channel. Both peers
//! may be blocked writing a large page at the same moment; if neither was
//! reading, that would deadlock once the QUIC flow-control window fills.
//! The protocol is lock-step per direction (one outstanding `Pull`, one
//! outstanding `Changes`), so at most a handful of messages are ever queued.

use std::sync::Arc;
use std::time::Duration;

use iroh::{PublicKey, SecretKey};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::{Mutex, mpsc};

use crate::change::{ChangeSource, ChangeSourceError, WireCursor};
use crate::protocol::{
    ErrorCode, FrameReader, FrameWriter, Message, PROTO_VERSION, ProtocolError, Role,
};

/// Tunables for [`SyncSession`].
#[derive(Debug, Clone)]
pub struct SessionConfig {
    /// Rows requested per `Pull` (default 500).
    pub page_size: usize,
    /// Upper bound on the `limit` a peer may ask us to serve (default 5000).
    pub max_page_size: usize,
    /// Bound on each wait for a peer message and on each frame write
    /// (default 30 s). The peer applies a page before asking for the next one,
    /// so this must cover its slowest `apply`.
    pub message_timeout: Duration,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            page_size: 500,
            max_page_size: 5000,
            message_timeout: Duration::from_secs(30),
        }
    }
}

/// What a finished session moved.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SessionReport {
    /// The peer's cr-sqlite site id from its `Hello`.
    pub peer_site_id: Vec<u8>,
    pub pages_pulled: usize,
    pub changes_pulled: usize,
    pub pages_served: usize,
    pub changes_served: usize,
    /// Persisted cursor for the peer when the session ended.
    pub cursor: WireCursor,
}

#[derive(Debug, thiserror::Error)]
pub enum SessionError {
    #[error(transparent)]
    Protocol(#[from] ProtocolError),
    #[error("change source: {0}")]
    Source(#[from] ChangeSourceError),
    #[error("timed out waiting for {0}")]
    Timeout(&'static str),
    #[error("peer closed the stream before the session completed")]
    Closed,
    #[error("peer reported {code:?}: {msg}")]
    Remote { code: ErrorCode, msg: String },
    #[error("protocol violation: {0}")]
    Violation(String),
    #[error("unsupported peer: {0}")]
    Unsupported(String),
}

impl SessionError {
    /// The code to tell the peer, if this failure is worth telling it about
    /// (not for transport failures, where there is nobody to tell).
    fn report_code(&self) -> Option<ErrorCode> {
        match self {
            SessionError::Source(_) | SessionError::Protocol(ProtocolError::FrameTooLarge(_)) => {
                Some(ErrorCode::Internal)
            }
            SessionError::Violation(_) | SessionError::Protocol(ProtocolError::Decode(_)) => {
                Some(ErrorCode::BadRequest)
            }
            SessionError::Unsupported(_) => Some(ErrorCode::Unsupported),
            _ => None,
        }
    }
}

/// One session with one peer. Build, then [`run`](Self::run) once.
pub struct SyncSession<S: ChangeSource> {
    source: Arc<S>,
    secret: SecretKey,
    peer: PublicKey,
    role: Role,
    config: SessionConfig,
    apply_lock: Arc<Mutex<()>>,
}

impl<S: ChangeSource> SyncSession<S> {
    pub fn new(
        source: Arc<S>,
        secret: SecretKey,
        peer: PublicKey,
        role: Role,
        config: SessionConfig,
    ) -> Self {
        Self {
            source,
            secret,
            peer,
            role,
            config,
            apply_lock: Arc::new(Mutex::new(())),
        }
    }

    /// Share one lock across all sessions with the same peer, so two
    /// concurrent sessions (we dialed while they dialed) cannot interleave
    /// `apply` + `set_peer_cursor` and move the stored cursor backwards.
    pub fn with_apply_lock(mut self, lock: Arc<Mutex<()>>) -> Self {
        self.apply_lock = lock;
        self
    }

    pub async fn run<R, W>(self, recv: R, send: W) -> Result<SessionReport, SessionError>
    where
        R: AsyncRead + Unpin + Send + 'static,
        W: AsyncWrite + Unpin + Send,
    {
        let writer = FrameWriter::new(send, self.secret.clone(), self.peer, self.role);
        let mut reader = FrameReader::new(recv, self.secret.clone(), self.peer, self.role);

        let (tx, rx) = mpsc::channel::<Result<Message, ProtocolError>>(16);
        let reader_task = tokio::spawn(async move {
            loop {
                match reader.recv().await {
                    Ok(Some(m)) => {
                        if tx.send(Ok(m)).await.is_err() {
                            break;
                        }
                    }
                    Ok(None) => break,
                    Err(e) => {
                        let _ = tx.send(Err(e)).await;
                        break;
                    }
                }
            }
        });
        let _abort = AbortOnDrop(reader_task);

        let mut io = Io {
            writer,
            rx,
            timeout: self.config.message_timeout,
        };

        let result = self.drive(&mut io).await;
        match &result {
            Ok(_) => {
                // Finish our half, then wait for the peer's EOF so we never
                // tear the connection down under its last read.
                let _ = io.writer.shutdown().await;
                let _ = tokio::time::timeout(io.timeout, async {
                    while io.rx.recv().await.is_some() {}
                })
                .await;
            }
            Err(e) => {
                if let Some(code) = e.report_code() {
                    // Never forward details: a source error may name tables,
                    // paths or SQL. The full error stays in our logs.
                    let msg = match code {
                        ErrorCode::Internal => "internal error".to_string(),
                        _ => e.to_string(),
                    };
                    let _ = tokio::time::timeout(
                        Duration::from_secs(2),
                        io.writer.send(&Message::Error { code, msg }),
                    )
                    .await;
                }
                let _ = tokio::time::timeout(io.timeout, io.writer.shutdown()).await;
            }
        }
        result
    }

    async fn drive<W: AsyncWrite + Unpin>(
        &self,
        io: &mut Io<W>,
    ) -> Result<SessionReport, SessionError> {
        let peer_str = self.peer.to_z32();
        let own_site = self.source.site_id().await?;

        io.send(&Message::Hello {
            device_id: self.secret.public().to_z32(),
            site_id: own_site.clone(),
            proto: PROTO_VERSION,
        })
        .await?;
        let peer_site = match io.next("peer hello").await? {
            Message::Hello {
                device_id,
                site_id,
                proto,
            } => {
                if proto != PROTO_VERSION {
                    return Err(SessionError::Unsupported(format!(
                        "protocol version {proto}, expected {PROTO_VERSION}"
                    )));
                }
                if device_id != peer_str {
                    return Err(SessionError::Unsupported(
                        "hello device id does not match the authenticated peer".into(),
                    ));
                }
                if site_id == own_site {
                    return Err(SessionError::Unsupported(
                        "peer has the same database site id as this device".into(),
                    ));
                }
                site_id
            }
            Message::Error { code, msg } => return Err(SessionError::Remote { code, msg }),
            _ => return Err(SessionError::Violation("expected Hello first".into())),
        };

        let page_size = self.config.page_size.max(1);
        let mut report = SessionReport {
            peer_site_id: peer_site.clone(),
            ..Default::default()
        };
        let mut after = self.source.peer_cursor(&peer_str).await?;
        report.cursor = after;

        io.send(&Message::Pull {
            after,
            limit: page_size as u32,
        })
        .await?;
        let mut outstanding: Option<WireCursor> = Some(after);
        let mut pull_done = false;
        let mut served_final: Option<WireCursor> = None;
        let mut serve_done = false;

        while !(pull_done && serve_done) {
            match io.next("peer message").await? {
                Message::Pull { after: from, limit } => {
                    // A new Pull reopens our serving direction until the page it
                    // produces is final and acked, even after an earlier final page.
                    serve_done = false;
                    let mut lim = (limit as usize).clamp(1, self.config.max_page_size.max(1));
                    loop {
                        let page = self
                            .source
                            .changes_since(from, Some(peer_site.clone()), lim)
                            .await?;
                        let msg = Message::Changes(page);
                        match io.send(&msg).await {
                            Ok(()) => {
                                let Message::Changes(page) = msg else {
                                    unreachable!()
                                };
                                report.pages_served += 1;
                                report.changes_served += page.changes.len();
                                served_final = (!page.more).then_some(page.next);
                                break;
                            }
                            Err(SessionError::Protocol(ProtocolError::FrameTooLarge(n)))
                                if lim > 1 =>
                            {
                                tracing::debug!(
                                    "sync-p2p: page of {lim} rows is {n} bytes; retrying smaller"
                                );
                                lim /= 2;
                            }
                            Err(e) => return Err(e),
                        }
                    }
                }
                Message::Changes(page) => {
                    let Some(asked) = outstanding.take() else {
                        return Err(SessionError::Violation("unsolicited Changes".into()));
                    };
                    if page.changes.len() > page_size {
                        return Err(SessionError::Violation(format!(
                            "page of {} changes exceeds the {page_size} requested",
                            page.changes.len()
                        )));
                    }
                    // A page must move the cursor unless it is the empty final page.
                    if page.next < asked
                        || (page.next == asked && (page.more || !page.changes.is_empty()))
                    {
                        return Err(SessionError::Violation(
                            "page does not advance the cursor".into(),
                        ));
                    }
                    let n = page.changes.len();
                    let next = page.next;
                    {
                        let _guard = self.apply_lock.lock().await;
                        let stored = self.source.peer_cursor(&peer_str).await?;
                        if next > stored {
                            if n > 0 {
                                self.source.apply(page.changes).await?;
                            }
                            self.source.set_peer_cursor(&peer_str, next).await?;
                            after = next;
                        } else {
                            after = stored;
                        }
                    }
                    report.pages_pulled += 1;
                    report.changes_pulled += n;
                    report.cursor = after;
                    io.send(&Message::Ack { cursor: next }).await?;
                    if page.more {
                        io.send(&Message::Pull {
                            after,
                            limit: page_size as u32,
                        })
                        .await?;
                        outstanding = Some(after);
                    } else {
                        pull_done = true;
                    }
                }
                Message::Ack { cursor } => {
                    if served_final == Some(cursor) {
                        serve_done = true;
                    }
                }
                Message::Error { code, msg } => return Err(SessionError::Remote { code, msg }),
                Message::Hello { .. } => {
                    return Err(SessionError::Violation("duplicate Hello".into()));
                }
            }
        }
        Ok(report)
    }
}

struct Io<W> {
    writer: FrameWriter<W>,
    rx: mpsc::Receiver<Result<Message, ProtocolError>>,
    timeout: Duration,
}

impl<W: AsyncWrite + Unpin> Io<W> {
    async fn send(&mut self, msg: &Message) -> Result<(), SessionError> {
        match tokio::time::timeout(self.timeout, self.writer.send(msg)).await {
            Ok(r) => Ok(r?),
            Err(_) => Err(SessionError::Timeout("writing to peer")),
        }
    }

    async fn next(&mut self, what: &'static str) -> Result<Message, SessionError> {
        match tokio::time::timeout(self.timeout, self.rx.recv()).await {
            Ok(Some(Ok(m))) => Ok(m),
            Ok(Some(Err(e))) => Err(e.into()),
            Ok(None) => Err(SessionError::Closed),
            Err(_) => Err(SessionError::Timeout(what)),
        }
    }
}

struct AbortOnDrop(tokio::task::JoinHandle<()>);

impl Drop for AbortOnDrop {
    fn drop(&mut self) {
        self.0.abort();
    }
}

use std::net::SocketAddr;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use crate::redirect::{RedirectOutcome, parse_redirect_request};

#[derive(Debug, thiserror::Error)]
pub enum LoopbackError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("port {0} is in use")]
    PortInUse(u16),
    #[error("oauth state mismatch")]
    StateMismatch,
    #[error("authorization denied: {0}")]
    AuthorizationDenied(String),
    #[error("redirect timed out")]
    Timeout,
    #[error("missing authorization code")]
    MissingCode,
    #[error("listener closed before receiving callback")]
    ListenerClosed,
}

pub enum LoopbackBindResult {
    Bound(LoopbackServer),
    PortInUse(u16),
}

pub struct LoopbackServer {
    listener: TcpListener,
    v6_listener: Option<TcpListener>,
    port: u16,
}

impl LoopbackServer {
    /// Attempts to bind to `127.0.0.1:port` and optionally `[::1]:port`.
    ///
    /// If `port == 0`, binds to an ephemeral port.
    /// If the requested port is already in use, returns `LoopbackBindResult::PortInUse`.
    pub async fn try_bind(port: u16) -> Result<LoopbackBindResult, std::io::Error> {
        let addr = SocketAddr::from(([127, 0, 0, 1], port));
        match TcpListener::bind(addr).await {
            Ok(listener) => {
                let actual_port = listener.local_addr()?.port();
                let v6_listener = TcpListener::bind(("::1", actual_port)).await.ok();
                Ok(LoopbackBindResult::Bound(Self {
                    listener,
                    v6_listener,
                    port: actual_port,
                }))
            }
            Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
                Ok(LoopbackBindResult::PortInUse(port))
            }
            Err(e) => Err(e),
        }
    }

    /// Bound local port.
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Listen for the OAuth redirect callback synchronously within timeout.
    pub async fn wait_for_redirect(
        self,
        expected_state: &str,
        timeout: Duration,
    ) -> Result<String, LoopbackError> {
        tokio::time::timeout(
            timeout,
            handle_connections_dual(self.listener, self.v6_listener, expected_state),
        )
        .await
        .map_err(|_| LoopbackError::Timeout)?
    }

    /// Spawn background listener task returning a oneshot receiver for the outcome.
    pub fn spawn(
        self,
        expected_state: String,
        timeout: Duration,
    ) -> tokio::sync::oneshot::Receiver<Result<String, LoopbackError>> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        tokio::spawn(async move {
            let res = tokio::time::timeout(
                timeout,
                handle_connections_dual(self.listener, self.v6_listener, &expected_state),
            )
            .await;
            let outcome = match res {
                Ok(inner) => inner,
                Err(_) => Err(LoopbackError::Timeout),
            };
            let _ = tx.send(outcome);
        });
        rx
    }
}

const SUCCESS_PAGE: &str = "<!doctype html><html><head><title>Notare</title></head><body style=\"font-family: sans-serif; display: flex; justify-content: center; margin-top: 20vh;\"><div style=\"text-align: center;\"><h2>Connected</h2><p>You can close this tab and return to Notare.</p></div></body></html>";

const FAILURE_PAGE: &str = "<!doctype html><html><head><title>Notare</title></head><body style=\"font-family: sans-serif; display: flex; justify-content: center; margin-top: 20vh;\"><div style=\"text-align: center;\"><h2>Connection failed</h2><p>Authorization was not granted. You can close this tab and try again from Notare.</p></div></body></html>";

const NOT_FOUND_RESPONSE: &str =
    "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";

fn html_response(body: &str) -> String {
    format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    )
}

/// Listen for OAuth redirect callbacks on a single IPv4 listener.
pub async fn handle_connections(
    listener: TcpListener,
    expected_state: &str,
) -> Result<String, LoopbackError> {
    handle_connections_dual(listener, None, expected_state).await
}

/// Listen for OAuth redirect callbacks on dual IPv4 and optional IPv6 listeners.
///
/// State mismatch behavior:
/// If a redirect arrives with an authorization code but missing or mismatched state,
/// the server responds with a failure page and immediately terminates with `LoopbackError::StateMismatch`.
/// Rationale: Failing fast protects against CSRF attempts and session corruption,
/// alerting the client immediately rather than hanging silently until timeout.
pub async fn handle_connections_dual(
    v4: TcpListener,
    v6: Option<TcpListener>,
    expected_state: &str,
) -> Result<String, LoopbackError> {
    loop {
        let mut stream = match &v6 {
            Some(v6_listener) => {
                tokio::select! {
                    res4 = v4.accept() => res4?.0,
                    res6 = v6_listener.accept() => res6?.0,
                }
            }
            None => v4.accept().await?.0,
        };

        let mut buf = vec![0u8; 8192];
        let mut read_total = 0;
        let mut found_crlf = false;
        while read_total < buf.len() {
            let n = stream.read(&mut buf[read_total..]).await?;
            if n == 0 {
                break;
            }
            read_total += n;
            if buf[..read_total].windows(2).any(|w| w == b"\r\n") {
                found_crlf = true;
                break;
            }
        }

        if !found_crlf && read_total == buf.len() {
            let _ = stream
                .write_all(b"HTTP/1.1 414 URI Too Long\r\nConnection: close\r\n\r\n")
                .await;
            let _ = stream.shutdown().await;
            continue;
        }

        if read_total == 0 {
            continue;
        }

        let head = match std::str::from_utf8(&buf[..read_total]) {
            Ok(s) => s.to_string(),
            Err(_) => {
                let _ = stream
                    .write_all(b"HTTP/1.1 400 Bad Request\r\nConnection: close\r\n\r\n")
                    .await;
                let _ = stream.shutdown().await;
                continue;
            }
        };

        match parse_redirect_request(&head) {
            RedirectOutcome::Code { code, state } => {
                if state != expected_state {
                    let _ = stream
                        .write_all(html_response(FAILURE_PAGE).as_bytes())
                        .await;
                    let _ = stream.shutdown().await;
                    return Err(LoopbackError::StateMismatch);
                }
                let _ = stream
                    .write_all(html_response(SUCCESS_PAGE).as_bytes())
                    .await;
                let _ = stream.shutdown().await;
                return Ok(code);
            }
            RedirectOutcome::MissingState => {
                let _ = stream
                    .write_all(html_response(FAILURE_PAGE).as_bytes())
                    .await;
                let _ = stream.shutdown().await;
                return Err(LoopbackError::StateMismatch);
            }
            RedirectOutcome::Error(error) => {
                let _ = stream
                    .write_all(html_response(FAILURE_PAGE).as_bytes())
                    .await;
                let _ = stream.shutdown().await;
                return Err(LoopbackError::AuthorizationDenied(error));
            }
            RedirectOutcome::Unrelated => {
                let _ = stream.write_all(NOT_FOUND_RESPONSE.as_bytes()).await;
                let _ = stream.shutdown().await;
            }
        }
    }
}

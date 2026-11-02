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
    port: u16,
}

impl LoopbackServer {
    /// Attempts to bind to `127.0.0.1:port`.
    ///
    /// If `port == 0`, binds to an ephemeral port.
    /// If the requested port is already in use, returns `LoopbackBindResult::PortInUse`.
    pub async fn try_bind(port: u16) -> Result<LoopbackBindResult, std::io::Error> {
        let addr = SocketAddr::from(([127, 0, 0, 1], port));
        match TcpListener::bind(addr).await {
            Ok(listener) => {
                let actual_port = listener.local_addr()?.port();
                Ok(LoopbackBindResult::Bound(Self {
                    listener,
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
        tokio::time::timeout(timeout, handle_connections(self.listener, expected_state))
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
            let res =
                tokio::time::timeout(timeout, handle_connections(self.listener, &expected_state))
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

pub async fn handle_connections(
    listener: TcpListener,
    expected_state: &str,
) -> Result<String, LoopbackError> {
    loop {
        let (mut stream, _addr) = listener.accept().await?;

        let mut buf = vec![0u8; 8192];
        let mut read_total = 0;
        let head = loop {
            let n = stream.read(&mut buf[read_total..]).await?;
            if n == 0 {
                break String::new();
            }
            read_total += n;
            let text = String::from_utf8_lossy(&buf[..read_total]);
            if text.contains("\r\n") || read_total == buf.len() {
                break text.into_owned();
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

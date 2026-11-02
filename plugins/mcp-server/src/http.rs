use std::net::SocketAddr;

use axum::{
    Router,
    extract::State,
    http::{StatusCode, header::AUTHORIZATION},
    middleware::{self, Next},
    response::Response,
};
use tokio::net::TcpListener;
use tokio::sync::watch;

use crate::error::{Error, Result};
use crate::server::NotareMcpServer;

pub async fn run_server(
    server: NotareMcpServer,
    port: u16,
    bearer_token: String,
    mut shutdown_rx: watch::Receiver<bool>,
    ready_tx: Option<tokio::sync::oneshot::Sender<Result<()>>>,
) -> Result<()> {
    if bearer_token.trim().is_empty() {
        let err_msg = "bearer token cannot be empty".to_string();
        if let Some(tx) = ready_tx {
            let _ = tx.send(Err(Error::Internal(err_msg.clone())));
        }
        return Err(Error::Internal(err_msg));
    }

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let listener = match TcpListener::bind(addr).await {
        Ok(l) => {
            if let Some(tx) = ready_tx {
                let _ = tx.send(Ok(()));
            }
            l
        }
        Err(e) => {
            let is_addr_in_use = e.kind() == std::io::ErrorKind::AddrInUse;
            let err_str = e.to_string();
            if let Some(tx) = ready_tx {
                let send_err = if is_addr_in_use {
                    Error::PortInUse(port)
                } else {
                    Error::Server(err_str.clone())
                };
                let _ = tx.send(Err(send_err));
            }
            return Err(if is_addr_in_use {
                Error::PortInUse(port)
            } else {
                Error::Server(err_str)
            });
        }
    };

    let mcp_service = hypr_mcp::create_service(move || Ok(server.clone()));

    let app = Router::new()
        .fallback_service(mcp_service)
        .layer(middleware::from_fn_with_state(
            bearer_token,
            require_bearer_auth,
        ));

    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            while !*shutdown_rx.borrow() {
                if shutdown_rx.changed().await.is_err() {
                    break;
                }
            }
        })
        .await
        .map_err(|e| Error::Server(e.to_string()))?;

    Ok(())
}

async fn require_bearer_auth(
    State(expected_token): State<String>,
    req: axum::extract::Request,
    next: Next,
) -> Result<Response, StatusCode> {
    if expected_token.trim().is_empty() {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let auth_header = req
        .headers()
        .get(AUTHORIZATION)
        .and_then(|h| h.to_str().ok());

    match auth_header {
        Some(h) if h.strip_prefix("Bearer ") == Some(&expected_token) => Ok(next.run(req).await),
        _ => Err(StatusCode::UNAUTHORIZED),
    }
}

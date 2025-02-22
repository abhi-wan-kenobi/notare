use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Query, State as AxumState,
    },
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use std::net::{Ipv4Addr, SocketAddr};

#[derive(Clone)]
pub struct ServerHandle {
    pub addr: SocketAddr,
    shutdown: tokio::sync::watch::Sender<()>,
}

pub async fn run_server(state: crate::SharedState) -> anyhow::Result<ServerHandle> {
    let app = Router::new()
        .route("/health", get(health))
        // should match our app server
        .route("/api/native/listen/realtime", get(listen))
        .with_state(state);

    let listener =
        tokio::net::TcpListener::bind(SocketAddr::from((Ipv4Addr::UNSPECIFIED, 0))).await?;

    let server_addr = listener.local_addr()?;

    let (shutdown_tx, mut shutdown_rx) = tokio::sync::watch::channel(());

    let server_handle = ServerHandle {
        addr: server_addr,
        shutdown: shutdown_tx,
    };

    tokio::spawn(async move {
        axum::serve(listener, app)
            .with_graceful_shutdown(async move {
                shutdown_rx.changed().await.ok();
            })
            .await
            .unwrap();
    });

    Ok(server_handle)
}

async fn health() -> impl IntoResponse {
    "ok"
}

async fn listen(
    Query(params): Query<hypr_bridge::ListenParams>,
    ws: WebSocketUpgrade,
    AxumState(state): AxumState<crate::SharedState>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| websocket(socket, state, params))
}

async fn websocket(
    socket: WebSocket,
    state: crate::SharedState,
    params: hypr_bridge::ListenParams,
) {
    tracing::info!("websocket_connected");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_health() {
        let state = crate::SharedState::default();
        let server = run_server(state).await.unwrap();

        let client = reqwest::Client::new();
        let response = client
            .get(format!("http://{}/health", server.addr))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_listen() {
        let state = crate::SharedState::default();
        {
            let mut state = state.lock().unwrap();
            state.model = Some(
                crate::model::model_builder(
                    dirs::home_dir()
                        .unwrap()
                        .join("Library/Application Support/com.hyprnote.dev/"),
                )
                .with_source(rwhisper::WhisperSource::QuantizedTiny)
                .build()
                .await
                .unwrap(),
            );
        }

        let server = run_server(state).await.unwrap();
        let client = reqwest::Client::new();
        let response = client
            .get(format!("http://{}/health", server.addr))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }
}

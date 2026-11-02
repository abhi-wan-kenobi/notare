//! Generic PKCE, loopback redirect listener, and paste fallback for OAuth flows.

mod paste;
mod pkce;
mod redirect;
mod server;

pub use paste::parse_paste_code;
pub use pkce::{PkcePair, generate_pkce, pkce_challenge_s256, verify_pkce};
pub use redirect::{RedirectOutcome, parse_redirect_request};
pub use server::{
    LoopbackBindResult, LoopbackError, LoopbackServer, handle_connections, handle_connections_dual,
};

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn loopback_server_handles_redirect() {
        let bind_result = LoopbackServer::try_bind(0).await.unwrap();
        let server = match bind_result {
            LoopbackBindResult::Bound(s) => s,
            LoopbackBindResult::PortInUse(_) => panic!("ephemeral port should not be in use"),
        };
        let port = server.port();
        let expected_state = "test_state_123";

        let rx = server.spawn(expected_state.to_string(), Duration::from_secs(5));

        // Send request from background task
        tokio::spawn(async move {
            let client = reqwest::Client::new();
            let _ = client
                .get(format!(
                    "http://127.0.0.1:{port}/callback?code=the_secret_code&state={expected_state}"
                ))
                .send()
                .await;
        });

        let outcome = rx.await.unwrap().unwrap();
        assert_eq!(outcome, "the_secret_code");
    }

    #[tokio::test]
    async fn loopback_server_rejects_state_mismatch() {
        let bind_result = LoopbackServer::try_bind(0).await.unwrap();
        let server = match bind_result {
            LoopbackBindResult::Bound(s) => s,
            LoopbackBindResult::PortInUse(_) => panic!("ephemeral port should not be in use"),
        };
        let port = server.port();

        let rx = server.spawn("state_abc".to_string(), Duration::from_secs(5));

        tokio::spawn(async move {
            let client = reqwest::Client::new();
            let _ = client
                .get(format!(
                    "http://127.0.0.1:{port}/callback?code=the_secret_code&state=wrong_state"
                ))
                .send()
                .await;
        });

        let outcome = rx.await.unwrap();
        assert!(matches!(outcome, Err(LoopbackError::StateMismatch)));
    }

    #[tokio::test]
    async fn loopback_server_rejects_missing_state() {
        let bind_result = LoopbackServer::try_bind(0).await.unwrap();
        let server = match bind_result {
            LoopbackBindResult::Bound(s) => s,
            LoopbackBindResult::PortInUse(_) => panic!("ephemeral port should not be in use"),
        };
        let port = server.port();

        let rx = server.spawn("expected_state".to_string(), Duration::from_secs(5));

        tokio::spawn(async move {
            let client = reqwest::Client::new();
            let _ = client
                .get(format!("http://127.0.0.1:{port}/callback?code=some_code"))
                .send()
                .await;
        });

        let outcome = rx.await.unwrap();
        assert!(matches!(outcome, Err(LoopbackError::StateMismatch)));
    }

    #[tokio::test]
    async fn loopback_server_handles_ipv6_redirect() {
        let bind_result = LoopbackServer::try_bind(0).await.unwrap();
        let server = match bind_result {
            LoopbackBindResult::Bound(s) => s,
            LoopbackBindResult::PortInUse(_) => panic!("ephemeral port should not be in use"),
        };
        let port = server.port();
        let expected_state = "ipv6_state_123";

        let rx = server.spawn(expected_state.to_string(), Duration::from_secs(5));

        tokio::spawn(async move {
            let client = reqwest::Client::new();
            // Try hitting IPv6 ::1; if IPv6 isn't routable in test environment, fall back to 127.0.0.1
            let v6_url =
                format!("http://[::1]:{port}/callback?code=v6_secret_code&state={expected_state}");
            if client.get(&v6_url).send().await.is_err() {
                let _ = client
                    .get(format!(
                        "http://127.0.0.1:{port}/callback?code=v6_secret_code&state={expected_state}"
                    ))
                    .send()
                    .await;
            }
        });

        let outcome = rx.await.unwrap().unwrap();
        assert_eq!(outcome, "v6_secret_code");
    }

    #[tokio::test]
    async fn loopback_server_handles_partial_reads() {
        use tokio::io::AsyncWriteExt;
        let bind_result = LoopbackServer::try_bind(0).await.unwrap();
        let server = match bind_result {
            LoopbackBindResult::Bound(s) => s,
            LoopbackBindResult::PortInUse(_) => panic!("ephemeral port should not be in use"),
        };
        let port = server.port();
        let expected_state = "state_partial_xyz";

        let rx = server.spawn(expected_state.to_string(), Duration::from_secs(5));

        tokio::spawn(async move {
            let mut stream = tokio::net::TcpStream::connect(("127.0.0.1", port))
                .await
                .unwrap();
            // Send request in two chunks with a brief delay
            stream
                .write_all(b"GET /callback?code=chunked_code")
                .await
                .unwrap();
            tokio::time::sleep(Duration::from_millis(50)).await;
            stream
                .write_all(
                    format!("&state={expected_state} HTTP/1.1\r\nHost: localhost\r\n\r\n")
                        .as_bytes(),
                )
                .await
                .unwrap();
        });

        let outcome = rx.await.unwrap().unwrap();
        assert_eq!(outcome, "chunked_code");
    }
}

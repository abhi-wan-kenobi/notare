//! Generic PKCE, loopback redirect listener, and paste fallback for OAuth flows.

mod paste;
mod pkce;
mod redirect;
mod server;

pub use paste::parse_paste_code;
pub use pkce::{PkcePair, generate_pkce, pkce_challenge_s256, verify_pkce};
pub use redirect::{RedirectOutcome, parse_redirect_request};
pub use server::{LoopbackBindResult, LoopbackError, LoopbackServer, handle_connections};

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
}

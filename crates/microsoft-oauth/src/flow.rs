use std::time::Duration;
use tokio::net::TcpListener;

pub use hypr_oauth_loopback::{
    LoopbackError, PkcePair, RedirectOutcome, generate_pkce, parse_redirect_request,
    pkce_challenge_s256,
};

use crate::error::Error;
use crate::token::{TokenResponse, exchange_code, fetch_user_email};

pub const DEFAULT_AUTH_URI: &str = "https://login.microsoftonline.com/common/oauth2/v2.0/authorize";
pub const DEFAULT_TOKEN_URI: &str = "https://login.microsoftonline.com/common/oauth2/v2.0/token";
pub const DEFAULT_GRAPH_BASE_URL: &str = "https://graph.microsoft.com/v1.0";

/// Scopes required for reading user info and calendars.
pub const DEFAULT_READ_SCOPES: &[&str] = &["offline_access", "User.Read", "Calendars.Read"];

/// Incremental write scope required for creating/modifying calendar events.
pub const WRITE_EVENTS_SCOPE: &str = "Calendars.ReadWrite";

/// Combined scopes for full read + write calendar access.
pub const DEFAULT_WRITE_SCOPES: &[&str] = &[
    "offline_access",
    "User.Read",
    "Calendars.Read",
    "Calendars.ReadWrite",
];

/// Build the consent-screen URL for Microsoft Identity platform.
///
/// Uses `response_mode=query` and PKCE `code_challenge_method=S256`.
pub fn build_auth_url(
    auth_uri: &str,
    client_id: &str,
    redirect_uri: &str,
    scopes: &[&str],
    state: &str,
    code_challenge: &str,
    prompt: Option<&str>,
) -> String {
    let scope = scopes.join(" ");
    let mut params = vec![
        ("client_id", client_id),
        ("response_type", "code"),
        ("redirect_uri", redirect_uri),
        ("response_mode", "query"),
        ("scope", &scope),
        ("state", state),
        ("code_challenge", code_challenge),
        ("code_challenge_method", "S256"),
    ];
    if let Some(p) = prompt {
        params.push(("prompt", p));
    }

    let query = params
        .iter()
        .map(|(k, v)| format!("{}={}", k, urlencoding::encode(v)))
        .collect::<Vec<_>>()
        .join("&");
    format!("{}?{}", auth_uri, query)
}

async fn handle_connections(listener: TcpListener, expected_state: &str) -> Result<String, Error> {
    hypr_oauth_loopback::handle_connections(listener, expected_state)
        .await
        .map_err(|e| match e {
            LoopbackError::StateMismatch => Error::StateMismatch,
            LoopbackError::AuthorizationDenied(err) => Error::AuthorizationDenied(err),
            LoopbackError::Io(err) => Error::Io(err),
            other => Error::AuthorizationDenied(other.to_string()),
        })
}

/// Run loopback authorization and code exchange, returning tokens and the user's email.
#[allow(clippy::too_many_arguments)] // mirrors google-oauth::connect; callers are two plugin fns
pub async fn connect(
    http: &reqwest::Client,
    client_id: &str,
    scopes: &[&str],
    timeout: Duration,
    auth_uri: &str,
    token_uri: &str,
    graph_base_url: &str,
    open_browser: impl FnOnce(&str) -> Result<(), String>,
) -> Result<(TokenResponse, String), Error> {
    let listener = TcpListener::bind(("127.0.0.1", 0)).await?;
    let port = listener.local_addr()?.port();
    // Microsoft requires localhost (not 127.0.0.1) for public desktop clients.
    let redirect_uri = format!("http://localhost:{port}");

    let state = uuid::Uuid::new_v4().simple().to_string();
    let pkce = generate_pkce();
    let auth_url = build_auth_url(
        auth_uri,
        client_id,
        &redirect_uri,
        scopes,
        &state,
        &pkce.challenge,
        None,
    );

    open_browser(&auth_url).map_err(Error::OpenBrowser)?;

    let code = tokio::time::timeout(timeout, handle_connections(listener, &state))
        .await
        .map_err(|_| Error::RedirectTimeout)??;

    let token = exchange_code(
        http,
        token_uri,
        client_id,
        &code,
        &redirect_uri,
        &pkce.verifier,
    )
    .await?;

    let email = fetch_user_email(http, graph_base_url, &token.access_token).await?;

    Ok((token, email))
}

/// Run loopback authorization with incremental scope (e.g. `Calendars.ReadWrite`).
pub async fn connect_incremental(
    http: &reqwest::Client,
    client_id: &str,
    scopes: &[&str],
    timeout: Duration,
    auth_uri: &str,
    token_uri: &str,
    open_browser: impl FnOnce(&str) -> Result<(), String>,
) -> Result<TokenResponse, Error> {
    let listener = TcpListener::bind(("127.0.0.1", 0)).await?;
    let port = listener.local_addr()?.port();
    let redirect_uri = format!("http://localhost:{port}");

    let state = uuid::Uuid::new_v4().simple().to_string();
    let pkce = generate_pkce();
    let auth_url = build_auth_url(
        auth_uri,
        client_id,
        &redirect_uri,
        scopes,
        &state,
        &pkce.challenge,
        Some("consent"),
    );

    open_browser(&auth_url).map_err(Error::OpenBrowser)?;

    let code = tokio::time::timeout(timeout, handle_connections(listener, &state))
        .await
        .map_err(|_| Error::RedirectTimeout)??;

    let token = exchange_code(
        http,
        token_uri,
        client_id,
        &code,
        &redirect_uri,
        &pkce.verifier,
    )
    .await?;

    Ok(token)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_auth_url_query_parameters() {
        let auth_url = build_auth_url(
            DEFAULT_AUTH_URI,
            "ms-client-123",
            "http://localhost:54321",
            DEFAULT_READ_SCOPES,
            "random_state",
            "challenge_s256",
            None,
        );

        assert!(auth_url.starts_with(DEFAULT_AUTH_URI));
        assert!(auth_url.contains("client_id=ms-client-123"));
        assert!(auth_url.contains("redirect_uri=http%3A%2F%2Flocalhost%3A54321"));
        assert!(auth_url.contains("response_type=code"));
        assert!(auth_url.contains("response_mode=query"));
        assert!(auth_url.contains("code_challenge=challenge_s256"));
        assert!(auth_url.contains("code_challenge_method=S256"));
        assert!(auth_url.contains("offline_access"));
        assert!(auth_url.contains("User.Read"));
        assert!(auth_url.contains("Calendars.Read"));
        assert!(!auth_url.contains("prompt="));
    }

    #[test]
    fn test_build_incremental_auth_url_contains_prompt_consent() {
        let auth_url = build_auth_url(
            DEFAULT_AUTH_URI,
            "ms-client-123",
            "http://localhost:54321",
            DEFAULT_WRITE_SCOPES,
            "random_state",
            "challenge_s256",
            Some("consent"),
        );

        assert!(auth_url.contains("prompt=consent"));
        assert!(auth_url.contains("Calendars.ReadWrite"));
    }

    #[tokio::test]
    async fn test_full_connect_flow_with_mock_servers() {
        use wiremock::matchers::{header, method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let token_server = MockServer::start().await;
        let graph_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "access_token": "at-mock",
                "refresh_token": "rt-mock",
                "expires_in": 3600,
                "scope": "offline_access User.Read Calendars.Read"
            })))
            .mount(&token_server)
            .await;

        Mock::given(method("GET"))
            .and(path("/me"))
            .and(header("authorization", "Bearer at-mock"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "mail": "user@outlook.com",
                "userPrincipalName": "user@outlook.com"
            })))
            .mount(&graph_server)
            .await;

        let http = reqwest::Client::new();
        let (tx, rx) = tokio::sync::oneshot::channel::<String>();

        let token_uri = format!("{}/token", token_server.uri());
        let graph_uri = graph_server.uri();
        let connect_fut = connect(
            &http,
            "client-id",
            DEFAULT_READ_SCOPES,
            Duration::from_secs(5),
            "https://login.microsoftonline.com/auth",
            &token_uri,
            &graph_uri,
            move |url| {
                let _ = tx.send(url.to_string());
                Ok(())
            },
        );

        let simulate_redirect = async {
            let auth_url = rx.await.unwrap();
            let parsed = reqwest::Url::parse(&auth_url).unwrap();
            let mut state = String::new();
            let mut redirect_uri = String::new();
            for (k, v) in parsed.query_pairs() {
                if k == "state" {
                    state = v.into_owned();
                } else if k == "redirect_uri" {
                    redirect_uri = v.into_owned();
                }
            }
            let callback_url = format!("{redirect_uri}?code=auth-code-123&state={state}");
            let client = reqwest::Client::new();
            let _ = client.get(&callback_url).send().await;
        };

        let (connect_res, _) = tokio::join!(connect_fut, simulate_redirect);
        let (token, email) = connect_res.unwrap();
        assert_eq!(token.access_token, "at-mock");
        assert_eq!(token.refresh_token.as_deref(), Some("rt-mock"));
        assert_eq!(email, "user@outlook.com");
    }
}

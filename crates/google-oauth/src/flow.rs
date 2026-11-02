use std::time::Duration;

use tokio::net::TcpListener;

pub use hypr_oauth_loopback::{
    PkcePair, RedirectOutcome, generate_pkce, parse_redirect_request, pkce_challenge_s256,
};

use crate::client_json::ClientCredentials;
use crate::error::Error;
use crate::token::{TokenResponse, exchange_code};

/// Read-only access to the calendar list + events, plus the account email so the
/// UI can show which Google account is connected.
pub const DEFAULT_SCOPES: &[&str] = &[
    "https://www.googleapis.com/auth/calendar.readonly",
    "https://www.googleapis.com/auth/calendar.events.readonly",
    "https://www.googleapis.com/auth/userinfo.email",
];

/// Build the consent-screen URL for the installed-app (loopback) flow.
pub fn build_auth_url(
    creds: &ClientCredentials,
    redirect_uri: &str,
    scopes: &[&str],
    state: &str,
    code_challenge: &str,
) -> String {
    let scope = scopes.join(" ");
    let params = [
        ("client_id", creds.client_id.as_str()),
        ("redirect_uri", redirect_uri),
        ("response_type", "code"),
        ("scope", &scope),
        // Ask for a refresh token, every time.
        ("access_type", "offline"),
        ("prompt", "consent"),
        ("state", state),
        ("code_challenge", code_challenge),
        ("code_challenge_method", "S256"),
    ];
    let query = params
        .iter()
        .map(|(k, v)| format!("{}={}", k, urlencoding::encode(v)))
        .collect::<Vec<_>>()
        .join("&");
    format!("{}?{}", creds.auth_uri, query)
}

async fn handle_connections(listener: TcpListener, expected_state: &str) -> Result<String, Error> {
    hypr_oauth_loopback::handle_connections(listener, expected_state)
        .await
        .map_err(|e| match e {
            hypr_oauth_loopback::LoopbackError::StateMismatch => Error::StateMismatch,
            hypr_oauth_loopback::LoopbackError::AuthorizationDenied(err) => {
                Error::AuthorizationDenied(err)
            }
            hypr_oauth_loopback::LoopbackError::Io(err) => Error::Io(err),
            other => Error::AuthorizationDenied(other.to_string()),
        })
}

/// Run the full installed-app OAuth flow:
/// bind a loopback listener → build the consent URL → `open_browser(url)` →
/// wait for the redirect → exchange the code for tokens.
///
/// `open_browser` is injected so the caller (the Tauri plugin) can use the
/// platform opener; this crate stays UI-free and testable.
pub async fn connect(
    http: &reqwest::Client,
    creds: &ClientCredentials,
    scopes: &[&str],
    timeout: Duration,
    open_browser: impl FnOnce(&str) -> Result<(), String>,
) -> Result<TokenResponse, Error> {
    let listener = TcpListener::bind(("127.0.0.1", 0)).await?;
    let port = listener.local_addr()?.port();
    let redirect_uri = format!("http://127.0.0.1:{port}/");

    let state = uuid::Uuid::new_v4().simple().to_string();
    let pkce = generate_pkce();
    let auth_url = build_auth_url(creds, &redirect_uri, scopes, &state, &pkce.challenge);

    open_browser(&auth_url).map_err(Error::OpenBrowser)?;

    let code = tokio::time::timeout(timeout, handle_connections(listener, &state))
        .await
        .map_err(|_| Error::RedirectTimeout)??;

    let token = exchange_code(http, creds, &code, &redirect_uri, &pkce.verifier).await?;
    if token.refresh_token.is_none() {
        return Err(Error::MissingRefreshToken);
    }
    Ok(token)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn creds() -> ClientCredentials {
        ClientCredentials {
            client_id: "1234-abc.apps.googleusercontent.com".to_string(),
            client_secret: "GOCSPX-secret".to_string(),
            auth_uri: crate::client_json::DEFAULT_AUTH_URI.to_string(),
            token_uri: crate::client_json::DEFAULT_TOKEN_URI.to_string(),
        }
    }

    #[test]
    fn builds_auth_url() {
        let url = build_auth_url(
            &creds(),
            "http://127.0.0.1:43210/",
            DEFAULT_SCOPES,
            "the-state",
            "the-challenge",
        );

        assert!(url.starts_with("https://accounts.google.com/o/oauth2/v2/auth?"));
        assert!(url.contains("client_id=1234-abc.apps.googleusercontent.com"));
        assert!(url.contains("redirect_uri=http%3A%2F%2F127.0.0.1%3A43210%2F"));
        assert!(url.contains("response_type=code"));
        assert!(url.contains(
            "scope=https%3A%2F%2Fwww.googleapis.com%2Fauth%2Fcalendar.readonly%20https%3A%2F%2Fwww.googleapis.com%2Fauth%2Fcalendar.events.readonly"
        ));
        assert!(url.contains("access_type=offline"));
        assert!(url.contains("prompt=consent"));
        assert!(url.contains("state=the-state"));
        assert!(url.contains("code_challenge=the-challenge"));
        assert!(url.contains("code_challenge_method=S256"));
        // The client secret must never appear in the browser URL.
        assert!(!url.contains("GOCSPX"));
    }

    #[test]
    fn pkce_challenge_matches_rfc7636_appendix_b() {
        // Test vector from RFC 7636 Appendix B.
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        assert_eq!(
            pkce_challenge_s256(verifier),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }

    #[test]
    fn generated_pkce_is_valid() {
        let pkce = generate_pkce();
        assert_eq!(pkce.verifier.len(), 64);
        assert!(pkce.verifier.chars().all(|c| c.is_ascii_hexdigit()));
        assert_eq!(pkce.challenge, pkce_challenge_s256(&pkce.verifier));
        // Unique per call.
        assert_ne!(generate_pkce().verifier, pkce.verifier);
    }

    #[test]
    fn parses_redirect_requests() {
        assert_eq!(
            parse_redirect_request("GET /?code=abc%2F123&state=xyz HTTP/1.1\r\nHost: x\r\n\r\n"),
            RedirectOutcome::Code {
                code: "abc/123".to_string(),
                state: "xyz".to_string()
            }
        );
        assert_eq!(
            parse_redirect_request("GET /?error=access_denied&state=xyz HTTP/1.1\r\n"),
            RedirectOutcome::Error("access_denied".to_string())
        );
        assert_eq!(
            parse_redirect_request("GET /favicon.ico HTTP/1.1\r\n"),
            RedirectOutcome::Unrelated
        );
        assert_eq!(parse_redirect_request(""), RedirectOutcome::Unrelated);
    }

    #[tokio::test]
    async fn full_flow_against_loopback_and_mock_token_endpoint() {
        use wiremock::matchers::{body_string_contains, method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/token"))
            .and(body_string_contains("grant_type=authorization_code"))
            .and(body_string_contains("code=the-code"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "access_token": "at-1",
                "expires_in": 3599,
                "refresh_token": "rt-1",
                "token_type": "Bearer"
            })))
            .mount(&server)
            .await;

        let mut creds = creds();
        creds.token_uri = format!("{}/token", server.uri());

        let http = reqwest::Client::new();
        let token = connect(
            &http,
            &creds,
            DEFAULT_SCOPES,
            Duration::from_secs(10),
            |auth_url| {
                // Simulate the browser: extract redirect_uri+state from the auth
                // URL and hit the loopback listener with a code.
                let redirect_uri = auth_url
                    .split("redirect_uri=")
                    .nth(1)
                    .unwrap()
                    .split('&')
                    .next()
                    .unwrap();
                let redirect_uri = urlencoding::decode(redirect_uri).unwrap().into_owned();
                let state = auth_url
                    .split("state=")
                    .nth(1)
                    .unwrap()
                    .split('&')
                    .next()
                    .unwrap()
                    .to_string();
                tokio::spawn(async move {
                    // An unrelated request first (favicon) must not kill the flow.
                    let _ = reqwest::get(format!("{redirect_uri}favicon.ico")).await;
                    let _ =
                        reqwest::get(format!("{redirect_uri}?code=the-code&state={state}")).await;
                });
                Ok(())
            },
        )
        .await
        .unwrap();

        assert_eq!(token.access_token, "at-1");
        assert_eq!(token.refresh_token.as_deref(), Some("rt-1"));
    }

    #[tokio::test]
    async fn rejects_state_mismatch() {
        let http = reqwest::Client::new();
        let result = connect(
            &http,
            &creds(),
            DEFAULT_SCOPES,
            Duration::from_secs(10),
            |auth_url| {
                let redirect_uri = auth_url
                    .split("redirect_uri=")
                    .nth(1)
                    .unwrap()
                    .split('&')
                    .next()
                    .unwrap();
                let redirect_uri = urlencoding::decode(redirect_uri).unwrap().into_owned();
                tokio::spawn(async move {
                    let _ = reqwest::get(format!("{redirect_uri}?code=x&state=WRONG")).await;
                });
                Ok(())
            },
        )
        .await;

        assert!(matches!(result, Err(Error::StateMismatch)));
    }
}

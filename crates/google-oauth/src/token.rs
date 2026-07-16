use crate::client_json::ClientCredentials;
use crate::error::Error;

/// Response from Google's token endpoint (code exchange or refresh).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TokenResponse {
    pub access_token: String,
    /// Lifetime in seconds (usually 3599).
    #[serde(default)]
    pub expires_in: Option<u64>,
    /// Only present on the initial code exchange (with `access_type=offline`),
    /// or when Google decides to rotate it.
    #[serde(default)]
    pub refresh_token: Option<String>,
    #[serde(default)]
    pub scope: Option<String>,
    #[serde(default)]
    pub token_type: Option<String>,
}

fn form_encode(pairs: &[(&str, &str)]) -> String {
    pairs
        .iter()
        .map(|(k, v)| format!("{}={}", k, urlencoding::encode(v)))
        .collect::<Vec<_>>()
        .join("&")
}

async fn post_token_endpoint(
    http: &reqwest::Client,
    token_uri: &str,
    body: String,
) -> Result<TokenResponse, Error> {
    let response = http
        .post(token_uri)
        .header(
            reqwest::header::CONTENT_TYPE,
            "application/x-www-form-urlencoded",
        )
        .body(body)
        .send()
        .await?;

    let status = response.status();
    let bytes = response.bytes().await?;

    if !status.is_success() {
        return Err(Error::TokenEndpoint {
            status,
            body: String::from_utf8_lossy(&bytes).into_owned(),
        });
    }

    Ok(serde_json::from_slice(&bytes)?)
}

/// Exchange an authorization code for tokens (installed-app flow, PKCE).
pub async fn exchange_code(
    http: &reqwest::Client,
    creds: &ClientCredentials,
    code: &str,
    redirect_uri: &str,
    pkce_verifier: &str,
) -> Result<TokenResponse, Error> {
    let body = form_encode(&[
        ("client_id", &creds.client_id),
        ("client_secret", &creds.client_secret),
        ("code", code),
        ("code_verifier", pkce_verifier),
        ("grant_type", "authorization_code"),
        ("redirect_uri", redirect_uri),
    ]);
    post_token_endpoint(http, &creds.token_uri, body).await
}

/// Get a fresh access token from a refresh token.
pub async fn refresh_access_token(
    http: &reqwest::Client,
    creds: &ClientCredentials,
    refresh_token: &str,
) -> Result<TokenResponse, Error> {
    let body = form_encode(&[
        ("client_id", &creds.client_id),
        ("client_secret", &creds.client_secret),
        ("grant_type", "refresh_token"),
        ("refresh_token", refresh_token),
    ]);
    post_token_endpoint(http, &creds.token_uri, body).await
}

pub const DEFAULT_REVOKE_URI: &str = "https://oauth2.googleapis.com/revoke";

/// Best-effort revocation of a refresh (or access) token.
pub async fn revoke_token(
    http: &reqwest::Client,
    revoke_uri: &str,
    token: &str,
) -> Result<(), Error> {
    let body = form_encode(&[("token", token)]);
    let response = http
        .post(revoke_uri)
        .header(
            reqwest::header::CONTENT_TYPE,
            "application/x-www-form-urlencoded",
        )
        .body(body)
        .send()
        .await?;

    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(Error::TokenEndpoint { status, body });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use wiremock::matchers::{body_string_contains, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;

    fn creds(token_uri: String) -> ClientCredentials {
        ClientCredentials {
            client_id: "id.apps.googleusercontent.com".to_string(),
            client_secret: "secret".to_string(),
            auth_uri: crate::client_json::DEFAULT_AUTH_URI.to_string(),
            token_uri,
        }
    }

    #[tokio::test]
    async fn exchanges_code() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/token"))
            .and(body_string_contains("grant_type=authorization_code"))
            .and(body_string_contains("code=the-code"))
            .and(body_string_contains("code_verifier=the-verifier"))
            .and(body_string_contains(
                "redirect_uri=http%3A%2F%2F127.0.0.1%3A9999%2F",
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "access_token": "at-1",
                "expires_in": 3599,
                "refresh_token": "rt-1",
                "scope": "https://www.googleapis.com/auth/calendar.readonly",
                "token_type": "Bearer"
            })))
            .mount(&server)
            .await;

        let http = reqwest::Client::new();
        let creds = creds(format!("{}/token", server.uri()));
        let token = exchange_code(
            &http,
            &creds,
            "the-code",
            "http://127.0.0.1:9999/",
            "the-verifier",
        )
        .await
        .unwrap();

        assert_eq!(token.access_token, "at-1");
        assert_eq!(token.refresh_token.as_deref(), Some("rt-1"));
        assert_eq!(token.expires_in, Some(3599));
    }

    #[tokio::test]
    async fn refreshes_token() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/token"))
            .and(body_string_contains("grant_type=refresh_token"))
            .and(body_string_contains("refresh_token=rt-1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "access_token": "at-2",
                "expires_in": 3599,
                "token_type": "Bearer"
            })))
            .mount(&server)
            .await;

        let http = reqwest::Client::new();
        let creds = creds(format!("{}/token", server.uri()));
        let token = refresh_access_token(&http, &creds, "rt-1").await.unwrap();

        assert_eq!(token.access_token, "at-2");
        assert!(token.refresh_token.is_none());
    }

    #[tokio::test]
    async fn surfaces_token_endpoint_errors() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/token"))
            .respond_with(ResponseTemplate::new(400).set_body_json(serde_json::json!({
                "error": "invalid_grant",
                "error_description": "Token has been expired or revoked."
            })))
            .mount(&server)
            .await;

        let http = reqwest::Client::new();
        let creds = creds(format!("{}/token", server.uri()));
        let err = refresh_access_token(&http, &creds, "rt-dead")
            .await
            .unwrap_err();

        match err {
            Error::TokenEndpoint { status, body } => {
                assert_eq!(status.as_u16(), 400);
                assert!(body.contains("invalid_grant"));
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }
}

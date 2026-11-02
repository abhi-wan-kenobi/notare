use crate::error::Error;
use std::time::Duration;

/// Default per-request timeout for token endpoint and Graph API calls.
pub const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// Response from Microsoft's token endpoint (code exchange or refresh).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct TokenResponse {
    pub access_token: String,
    /// Lifetime in seconds (typically 3599).
    #[serde(default)]
    pub expires_in: Option<u64>,
    #[serde(default)]
    pub ext_expires_in: Option<u64>,
    /// Only present on the initial code exchange or when refreshed.
    #[serde(default)]
    pub refresh_token: Option<String>,
    #[serde(default)]
    pub scope: Option<String>,
    #[serde(default)]
    pub token_type: Option<String>,
    #[serde(default)]
    pub id_token: Option<String>,
}

/// User profile from `GET https://graph.microsoft.com/v1.0/me`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct GraphUserProfile {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub mail: Option<String>,
    #[serde(default)]
    pub user_principal_name: Option<String>,
}

impl GraphUserProfile {
    /// Return the user's primary email address.
    /// Prefers `mail` if non-empty, falling back to `userPrincipalName`.
    pub fn email(&self) -> Option<String> {
        self.mail
            .as_ref()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(ToString::to_string)
            .or_else(|| {
                self.user_principal_name
                    .as_ref()
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty())
                    .map(ToString::to_string)
            })
    }
}

fn form_encode(pairs: &[(&str, &str)]) -> String {
    pairs
        .iter()
        .map(|(k, v)| format!("{}={}", urlencoding::encode(k), urlencoding::encode(v)))
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
        .timeout(DEFAULT_REQUEST_TIMEOUT)
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

    let parsed = serde_json::from_slice(&bytes)?;
    Ok(parsed)
}

/// Exchange an authorization code for tokens (public native desktop client, PKCE).
pub async fn exchange_code(
    http: &reqwest::Client,
    token_uri: &str,
    client_id: &str,
    code: &str,
    redirect_uri: &str,
    pkce_verifier: &str,
) -> Result<TokenResponse, Error> {
    if code.trim().is_empty() {
        return Err(Error::Api("empty authorization code".into()));
    }
    let body = form_encode(&[
        ("client_id", client_id),
        ("grant_type", "authorization_code"),
        ("code", code),
        ("redirect_uri", redirect_uri),
        ("code_verifier", pkce_verifier),
    ]);
    post_token_endpoint(http, token_uri, body).await
}

/// Get a fresh access token from a refresh token.
pub async fn refresh_access_token(
    http: &reqwest::Client,
    token_uri: &str,
    client_id: &str,
    refresh_token: &str,
    scopes: Option<&[&str]>,
) -> Result<TokenResponse, Error> {
    if refresh_token.trim().is_empty() {
        return Err(Error::Api("empty refresh token".into()));
    }
    let mut pairs = vec![
        ("client_id", client_id),
        ("grant_type", "refresh_token"),
        ("refresh_token", refresh_token),
    ];
    let joined_scopes;
    if let Some(s) = scopes {
        joined_scopes = s.join(" ");
        pairs.push(("scope", &joined_scopes));
    }
    let body = form_encode(&pairs);
    post_token_endpoint(http, token_uri, body).await
}

/// Fetch the user profile from Microsoft Graph (`/me`).
pub async fn fetch_user_profile(
    http: &reqwest::Client,
    graph_base_url: &str,
    access_token: &str,
) -> Result<GraphUserProfile, Error> {
    let url = format!("{}/me", graph_base_url.trim_end_matches('/'));
    let response = http
        .get(&url)
        .header(
            reqwest::header::AUTHORIZATION,
            format!("Bearer {access_token}"),
        )
        .timeout(DEFAULT_REQUEST_TIMEOUT)
        .send()
        .await?;

    let status = response.status();
    let bytes = response.bytes().await?;

    if !status.is_success() {
        return Err(Error::GraphEndpoint {
            status,
            body: String::from_utf8_lossy(&bytes).into_owned(),
        });
    }

    let parsed = serde_json::from_slice(&bytes)?;
    Ok(parsed)
}

/// Fetch user email address using `fetch_user_profile`.
pub async fn fetch_user_email(
    http: &reqwest::Client,
    graph_base_url: &str,
    access_token: &str,
) -> Result<String, Error> {
    let profile = fetch_user_profile(http, graph_base_url, access_token).await?;
    profile.email().ok_or(Error::MissingEmail)
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn exchanges_code_successfully() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/token"))
            .and(header("content-type", "application/x-www-form-urlencoded"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "token_type": "Bearer",
                "scope": "offline_access User.Read Calendars.Read",
                "expires_in": 3600,
                "ext_expires_in": 3600,
                "access_token": "at-123",
                "refresh_token": "rt-123"
            })))
            .mount(&server)
            .await;

        let http = reqwest::Client::new();
        let token = exchange_code(
            &http,
            &format!("{}/token", server.uri()),
            "client-id-1",
            "code-1",
            "http://localhost:54321",
            "verifier-1",
        )
        .await
        .unwrap();

        assert_eq!(token.access_token, "at-123");
        assert_eq!(token.refresh_token.as_deref(), Some("rt-123"));
        assert_eq!(token.expires_in, Some(3600));
    }

    #[tokio::test]
    async fn refreshes_access_token() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "token_type": "Bearer",
                "access_token": "at-fresh",
                "expires_in": 3600
            })))
            .mount(&server)
            .await;

        let http = reqwest::Client::new();
        let token = refresh_access_token(
            &http,
            &format!("{}/token", server.uri()),
            "client-id-1",
            "rt-123",
            None,
        )
        .await
        .unwrap();

        assert_eq!(token.access_token, "at-fresh");
    }

    #[tokio::test]
    async fn fetches_user_email_from_mail_field() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/me"))
            .and(header("authorization", "Bearer at-test"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "displayName": "Ada Lovelace",
                "mail": "ada@example.com",
                "userPrincipalName": "ada.lovelace@contoso.onmicrosoft.com"
            })))
            .mount(&server)
            .await;

        let http = reqwest::Client::new();
        let email = fetch_user_email(&http, &server.uri(), "at-test")
            .await
            .unwrap();
        assert_eq!(email, "ada@example.com");
    }

    #[tokio::test]
    async fn falls_back_to_user_principal_name_when_mail_is_null() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/me"))
            .and(header("authorization", "Bearer at-test"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "displayName": "Personal User",
                "mail": null,
                "userPrincipalName": "personal@outlook.com"
            })))
            .mount(&server)
            .await;

        let http = reqwest::Client::new();
        let email = fetch_user_email(&http, &server.uri(), "at-test")
            .await
            .unwrap();
        assert_eq!(email, "personal@outlook.com");
    }

    #[tokio::test]
    async fn surfaces_token_endpoint_errors() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/token"))
            .respond_with(ResponseTemplate::new(400).set_body_string("invalid_grant"))
            .mount(&server)
            .await;

        let http = reqwest::Client::new();
        let err = exchange_code(
            &http,
            &format!("{}/token", server.uri()),
            "client-id-1",
            "bad-code",
            "http://localhost:54321",
            "verifier",
        )
        .await
        .unwrap_err();

        assert!(matches!(err, Error::TokenEndpoint { .. }));
    }

    #[tokio::test]
    async fn rejects_empty_code_in_exchange() {
        let http = reqwest::Client::new();
        let err = exchange_code(
            &http,
            "http://127.0.0.1:9999/token",
            "cid",
            "  ",
            "http://localhost:54321",
            "verifier",
        )
        .await;
        assert!(matches!(err, Err(Error::Api(_))));
    }

    #[tokio::test]
    async fn rejects_empty_refresh_token() {
        let http = reqwest::Client::new();
        let err = refresh_access_token(&http, "http://127.0.0.1:9999/token", "cid", "", None).await;
        assert!(matches!(err, Err(Error::Api(_))));
    }
}

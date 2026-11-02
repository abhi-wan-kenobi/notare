use chrono::{Duration, Utc};
use serde_json::Value;

use crate::error::Error;
use crate::store::StoredTokens;

pub const CLIENT_ID: &str = "9d1c250a-e61b-44d9-88ed-5944d1962f5e";
pub const AUTH_URL: &str = "https://claude.ai/oauth/authorize";
pub const TOKEN_URL: &str = "https://platform.claude.com/v1/oauth/token";
pub const SCOPES: &str = "org:create_api_key user:profile user:inference";
pub const LOOPBACK_PORT: u16 = 53692;
pub const LOOPBACK_PATH: &str = "/callback";
pub const LOOPBACK_REDIRECT_URI: &str = "http://localhost:53692/callback";
pub const PASTE_REDIRECT_URI: &str = "https://platform.claude.com/oauth/code/callback";

pub fn build_auth_url(redirect_uri: &str, state: &str, challenge: &str) -> String {
    let params = [
        ("client_id", CLIENT_ID),
        ("redirect_uri", redirect_uri),
        ("response_type", "code"),
        ("scope", SCOPES),
        ("state", state),
        ("code_challenge", challenge),
        ("code_challenge_method", "S256"),
    ];

    let query = params
        .iter()
        .map(|(k, v)| format!("{}={}", k, urlencoding::encode(v)))
        .collect::<Vec<_>>()
        .join("&");

    format!("{AUTH_URL}?{query}")
}

pub async fn exchange_code(
    http: &reqwest::Client,
    code: &str,
    redirect_uri: &str,
    verifier: &str,
    state: &str,
) -> Result<StoredTokens, Error> {
    let body = serde_json::json!({
        "grant_type": "authorization_code",
        "client_id": CLIENT_ID,
        "code": code,
        "redirect_uri": redirect_uri,
        "code_verifier": verifier,
        "state": state
    });

    let resp = http
        .post(TOKEN_URL)
        .json(&body)
        .send()
        .await
        .map_err(|e| Error::TokenExchange(e.to_string()))?;

    if !resp.status().is_success() {
        let err_text = resp
            .text()
            .await
            .unwrap_or_else(|_| "unknown error".to_string());
        return Err(Error::TokenExchange(err_text));
    }

    let val: Value = resp
        .json()
        .await
        .map_err(|e| Error::TokenExchange(e.to_string()))?;

    parse_token_response(val)
}

pub async fn refresh_token(
    http: &reqwest::Client,
    refresh_token: &str,
    existing: &StoredTokens,
) -> Result<StoredTokens, Error> {
    let body = serde_json::json!({
        "grant_type": "refresh_token",
        "client_id": CLIENT_ID,
        "refresh_token": refresh_token,
    });

    let resp = http
        .post(TOKEN_URL)
        .json(&body)
        .send()
        .await
        .map_err(|e| Error::TokenExchange(e.to_string()))?;

    if !resp.status().is_success() {
        let err_text = resp
            .text()
            .await
            .unwrap_or_else(|_| "unknown error".to_string());
        return Err(Error::TokenExchange(err_text));
    }

    let val: Value = resp
        .json()
        .await
        .map_err(|e| Error::TokenExchange(e.to_string()))?;

    let mut new_tokens = parse_token_response(val)?;
    // If vendor doesn't return a new refresh token, keep the current one
    if new_tokens.refresh_token.is_none() {
        new_tokens.refresh_token = Some(refresh_token.to_string());
    }
    if new_tokens.account_label.is_none() {
        new_tokens.account_label = existing.account_label.clone();
    }
    new_tokens.extra = existing.extra.clone();

    Ok(new_tokens)
}

fn parse_token_response(val: Value) -> Result<StoredTokens, Error> {
    let access_token = val
        .get("access_token")
        .and_then(|v| v.as_str())
        .ok_or_else(|| Error::TokenExchange("missing access_token".to_string()))?
        .to_string();

    let refresh_token = val
        .get("refresh_token")
        .and_then(|v| v.as_str())
        .map(String::from);

    let expires_in = val.get("expires_in").and_then(|v| v.as_i64());
    let expires_at = expires_in.map(|sec| Utc::now() + Duration::seconds(sec));

    let account_label = val
        .get("email")
        .and_then(|v| v.as_str())
        .or_else(|| val.get("account_id").and_then(|v| v.as_str()))
        .map(String::from);

    Ok(StoredTokens {
        access_token,
        refresh_token,
        expires_at,
        account_label,
        extra: std::collections::HashMap::new(),
    })
}

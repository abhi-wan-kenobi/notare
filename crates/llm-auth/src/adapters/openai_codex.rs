use chrono::{Duration, Utc};
use serde_json::Value;
use std::collections::HashMap;

use crate::error::Error;
use crate::jwt::extract_openai_claims;
use crate::store::StoredTokens;

pub const CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";
pub const AUTH_URL: &str = "https://auth.openai.com/oauth/authorize";
pub const TOKEN_URL: &str = "https://auth.openai.com/oauth/token";
pub const REVOKE_URL: &str = "https://auth.openai.com/oauth/revoke";
pub const SCOPES: &str = "openid profile email offline_access";
pub const LOOPBACK_PORT: u16 = 1455;
pub const LOOPBACK_PATH: &str = "/auth/callback";
pub const LOOPBACK_REDIRECT_URI: &str = "http://localhost:1455/auth/callback";

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
fn form_encode(pairs: &[(&str, &str)]) -> String {
    pairs
        .iter()
        .map(|(k, v)| format!("{}={}", k, urlencoding::encode(v)))
        .collect::<Vec<_>>()
        .join("&")
}

pub async fn exchange_code(
    http: &reqwest::Client,
    code: &str,
    redirect_uri: &str,
    verifier: &str,
) -> Result<StoredTokens, Error> {
    let params = [
        ("grant_type", "authorization_code"),
        ("client_id", CLIENT_ID),
        ("code", code),
        ("code_verifier", verifier),
        ("redirect_uri", redirect_uri),
    ];

    let body = form_encode(&params);
    let resp = http
        .post(TOKEN_URL)
        .header(
            reqwest::header::CONTENT_TYPE,
            "application/x-www-form-urlencoded",
        )
        .body(body)
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
    let params = [
        ("grant_type", "refresh_token"),
        ("client_id", CLIENT_ID),
        ("refresh_token", refresh_token),
    ];

    let body = form_encode(&params);
    let resp = http
        .post(TOKEN_URL)
        .header(
            reqwest::header::CONTENT_TYPE,
            "application/x-www-form-urlencoded",
        )
        .body(body)
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
    if new_tokens.refresh_token.is_none() {
        new_tokens.refresh_token = Some(refresh_token.to_string());
    }
    if new_tokens.account_label.is_none() {
        new_tokens.account_label = existing.account_label.clone();
    }
    // Preserve existing extra claims (e.g. chatgpt_account_id if refresh doesn't include it)
    for (k, v) in &existing.extra {
        new_tokens
            .extra
            .entry(k.clone())
            .or_insert_with(|| v.clone());
    }

    Ok(new_tokens)
}

pub async fn revoke_token(http: &reqwest::Client, refresh_token: &str) -> Result<(), Error> {
    let params = [("client_id", CLIENT_ID), ("token", refresh_token)];
    let body = form_encode(&params);
    let _ = http
        .post(REVOKE_URL)
        .header(
            reqwest::header::CONTENT_TYPE,
            "application/x-www-form-urlencoded",
        )
        .body(body)
        .send()
        .await;
    Ok(())
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

    let id_token = val.get("id_token").and_then(|v| v.as_str());

    let expires_in = val.get("expires_in").and_then(|v| v.as_i64());
    let expires_at = expires_in.map(|sec| Utc::now() + Duration::seconds(sec));

    let mut extra = HashMap::new();
    let mut account_label = None;

    // Check id_token first, then access_token for OpenAI claims
    if let Some(id_tok) = id_token {
        let (chatgpt_id, label) = extract_openai_claims(id_tok);
        if let Some(id) = chatgpt_id {
            extra.insert("chatgpt_account_id".to_string(), id);
        }
        if label.is_some() {
            account_label = label;
        }
    }

    // If still missing chatgpt_account_id, check access_token JWT
    if !extra.contains_key("chatgpt_account_id") {
        let (chatgpt_id, label) = extract_openai_claims(&access_token);
        if let Some(id) = chatgpt_id {
            extra.insert("chatgpt_account_id".to_string(), id);
        }
        if account_label.is_none() && label.is_some() {
            account_label = label;
        }
    }

    Ok(StoredTokens {
        access_token,
        refresh_token,
        expires_at,
        account_label,
        extra,
    })
}

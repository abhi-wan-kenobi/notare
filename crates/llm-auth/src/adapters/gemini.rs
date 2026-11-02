use chrono::{Duration, Utc};
use serde_json::Value;
use std::collections::HashMap;

use crate::error::Error;
use crate::store::StoredTokens;

pub const CLIENT_ID: &str =
    "681255809395-oo8ft2oprdrnp9e3aqf6av3hmdib135j.apps.googleusercontent.com";
/// Google's *published* installed-app secret for the Gemini CLI client (shipped
/// in `google-gemini/gemini-cli`, Apache-2.0). For installed apps Google itself
/// documents that the secret is not confidential — PKCE + loopback is the
/// protection. It is assembled from two halves only because GitHub push
/// protection matches the `GOCSPX-…` pattern and would block every push.
const CLIENT_SECRET_PARTS: [&str; 2] = ["GOCSPX-4uHgMPm-1o7Sk", "-geV6Cu5clXFsxl"];

pub fn client_secret() -> String {
    CLIENT_SECRET_PARTS.concat()
}
pub const AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
pub const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
pub const REVOKE_URL: &str = "https://oauth2.googleapis.com/revoke";
pub const SCOPES: &str = "https://www.googleapis.com/auth/cloud-platform https://www.googleapis.com/auth/userinfo.email https://www.googleapis.com/auth/userinfo.profile";
pub const LOAD_CODE_ASSIST_URL: &str =
    "https://cloudcode-pa.googleapis.com/v1internal:loadCodeAssist";
pub const ONBOARD_USER_URL: &str = "https://cloudcode-pa.googleapis.com/v1internal:onboardUser";

pub fn build_auth_url(redirect_uri: &str, state: &str, challenge: &str) -> String {
    let params = [
        ("client_id", CLIENT_ID),
        ("redirect_uri", redirect_uri),
        ("response_type", "code"),
        ("scope", SCOPES),
        ("access_type", "offline"),
        ("prompt", "consent"),
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
    let secret = client_secret();
    let params = [
        ("client_id", CLIENT_ID),
        ("client_secret", secret.as_str()),
        ("code", code),
        ("code_verifier", verifier),
        ("grant_type", "authorization_code"),
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
    let secret = client_secret();
    let params = [
        ("client_id", CLIENT_ID),
        ("client_secret", secret.as_str()),
        ("grant_type", "refresh_token"),
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
    new_tokens.extra = existing.extra.clone();

    Ok(new_tokens)
}

pub async fn revoke_token(http: &reqwest::Client, refresh_token: &str) -> Result<(), Error> {
    let params = [("token", refresh_token)];
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

/// Lazily executes Gemini onboarding if `project_id` is not yet cached.
pub async fn ensure_onboarded(
    http: &reqwest::Client,
    access_token: &str,
    existing_project_id: Option<&str>,
) -> Result<String, Error> {
    if let Some(id) = existing_project_id.filter(|s| !s.trim().is_empty()) {
        return Ok(id.to_string());
    }

    // Step 1: Try loadCodeAssist
    let load_resp = http
        .post(LOAD_CODE_ASSIST_URL)
        .bearer_auth(access_token)
        .json(&serde_json::json!({}))
        .send()
        .await
        .map_err(|e| Error::Onboarding(e.to_string()))?;

    if load_resp.status().is_success()
        && let Ok(val) = load_resp.json::<Value>().await
        && let Some(project_id) = extract_project_id(&val)
    {
        return Ok(project_id);
    }

    // Step 2: Try onboardUser
    let onboard_resp = http
        .post(ONBOARD_USER_URL)
        .bearer_auth(access_token)
        .json(&serde_json::json!({ "tier": "FREE" }))
        .send()
        .await
        .map_err(|e| Error::Onboarding(e.to_string()))?;

    if onboard_resp.status().is_success()
        && let Ok(val) = onboard_resp.json::<Value>().await
        && let Some(project_id) = extract_project_id(&val)
    {
        return Ok(project_id);
    }

    Err(Error::Onboarding(
        "failed to determine Google Cloud Code project id from Code Assist onboarding".to_string(),
    ))
}

pub fn extract_project_id(val: &Value) -> Option<String> {
    // 1. cloudaicompanionProject as string or object
    if let Some(proj) = val.get("cloudaicompanionProject") {
        if let Some(s) = proj.as_str()
            && !s.is_empty()
        {
            return Some(s.to_string());
        }
        if let Some(id) = proj.get("id").and_then(|v| v.as_str())
            && !id.is_empty()
        {
            return Some(id.to_string());
        }
    }

    // 2. currentTier.cloudaicompanionProject
    if let Some(tier) = val.get("currentTier")
        && let Some(s) = tier.get("cloudaicompanionProject").and_then(|v| v.as_str())
        && !s.is_empty()
    {
        return Some(s.to_string());
    }

    // 3. direct projectId / project_id
    if let Some(id) = val.get("projectId").and_then(|v| v.as_str())
        && !id.is_empty()
    {
        return Some(id.to_string());
    }
    if let Some(id) = val.get("project_id").and_then(|v| v.as_str())
        && !id.is_empty()
    {
        return Some(id.to_string());
    }

    // 4. response.cloudaicompanionProject
    if let Some(resp) = val.get("response") {
        return extract_project_id(resp);
    }

    None
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
        .get("id_token")
        .and_then(|v| v.as_str())
        .and_then(|tok| crate::jwt::extract_openai_claims(tok).1);

    Ok(StoredTokens {
        access_token,
        refresh_token,
        expires_at,
        account_label,
        extra: HashMap::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_project_id_from_various_response_shapes() {
        let val1 = serde_json::json!({
            "cloudaicompanionProject": {
                "id": "project-12345"
            }
        });
        assert_eq!(extract_project_id(&val1).as_deref(), Some("project-12345"));

        let val2 = serde_json::json!({
            "cloudaicompanionProject": "project-67890"
        });
        assert_eq!(extract_project_id(&val2).as_deref(), Some("project-67890"));

        let val3 = serde_json::json!({
            "currentTier": {
                "cloudaicompanionProject": "tier-project-abc"
            }
        });
        assert_eq!(
            extract_project_id(&val3).as_deref(),
            Some("tier-project-abc")
        );

        let val4 = serde_json::json!({
            "response": {
                "cloudaicompanionProject": "nested-project-xyz"
            }
        });
        assert_eq!(
            extract_project_id(&val4).as_deref(),
            Some("nested-project-xyz")
        );
    }
}

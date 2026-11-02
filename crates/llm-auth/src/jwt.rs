use base64::Engine;
use serde_json::Value;

/// Extracts OpenAI-specific claims from an access_token or id_token JWT.
/// Returns `(chatgpt_account_id, email_or_account_label)`.
pub fn extract_openai_claims(token: &str) -> (Option<String>, Option<String>) {
    let payload = match decode_jwt_payload(token) {
        Some(p) => p,
        None => return (None, None),
    };

    let chatgpt_account_id = extract_chatgpt_account_id(&payload);
    let account_label = extract_account_label(&payload);

    (chatgpt_account_id, account_label)
}

fn decode_jwt_payload(token: &str) -> Option<Value> {
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() < 2 {
        return None;
    }

    let payload_b64 = parts[1];
    let decoded = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload_b64)
        .or_else(|_| {
            // Try with standard URL_SAFE (with padding)
            base64::engine::general_purpose::URL_SAFE.decode(payload_b64)
        })
        .ok()?;

    serde_json::from_slice(&decoded).ok()
}

fn extract_chatgpt_account_id(payload: &Value) -> Option<String> {
    // 1. Nested under "https://api.openai.com/auth" -> "chatgpt_account_id"
    if let Some(auth_obj) = payload.get("https://api.openai.com/auth")
        && let Some(id) = auth_obj.get("chatgpt_account_id").and_then(|v| v.as_str())
        && !id.is_empty()
    {
        return Some(id.to_string());
    }

    // 2. Direct top-level "chatgpt_account_id"
    if let Some(id) = payload.get("chatgpt_account_id").and_then(|v| v.as_str())
        && !id.is_empty()
    {
        return Some(id.to_string());
    }

    None
}

fn extract_account_label(payload: &Value) -> Option<String> {
    // Direct "email"
    if let Some(email) = payload.get("email").and_then(|v| v.as_str())
        && !email.is_empty()
    {
        return Some(email.to_string());
    }

    // "preferred_username"
    if let Some(name) = payload.get("preferred_username").and_then(|v| v.as_str())
        && !name.is_empty()
    {
        return Some(name.to_string());
    }

    // Nested in profile
    if let Some(profile) = payload.get("https://api.openai.com/profile")
        && let Some(email) = profile.get("email").and_then(|v| v.as_str())
        && !email.is_empty()
    {
        return Some(email.to_string());
    }

    // Fallback to "sub"
    if let Some(sub) = payload.get("sub").and_then(|v| v.as_str())
        && !sub.is_empty()
    {
        return Some(sub.to_string());
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_jwt(payload: &Value) -> String {
        let header = "eyJhbGciOiJSUzI1NiIsInR5cCI6IkpXVCJ9";
        let payload_str = serde_json::to_string(payload).unwrap();
        let payload_b64 =
            base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(payload_str.as_bytes());
        let signature = "fake_signature";
        format!("{header}.{payload_b64}.{signature}")
    }

    #[test]
    fn extracts_chatgpt_account_id_from_nested_auth_claim() {
        let payload = serde_json::json!({
            "https://api.openai.com/auth": {
                "chatgpt_account_id": "org-chatgpt-123456",
                "user_id": "user-abc"
            },
            "email": "user@example.com",
            "sub": "auth0|12345"
        });

        let jwt = make_test_jwt(&payload);
        let (account_id, label) = extract_openai_claims(&jwt);

        assert_eq!(account_id.as_deref(), Some("org-chatgpt-123456"));
        assert_eq!(label.as_deref(), Some("user@example.com"));
    }

    #[test]
    fn extracts_chatgpt_account_id_from_top_level() {
        let payload = serde_json::json!({
            "chatgpt_account_id": "org-top-level-789",
            "email": "dev@test.org"
        });

        let jwt = make_test_jwt(&payload);
        let (account_id, label) = extract_openai_claims(&jwt);

        assert_eq!(account_id.as_deref(), Some("org-top-level-789"));
        assert_eq!(label.as_deref(), Some("dev@test.org"));
    }

    #[test]
    fn handles_missing_claims_gracefully() {
        let payload = serde_json::json!({
            "sub": "user_123"
        });

        let jwt = make_test_jwt(&payload);
        let (account_id, label) = extract_openai_claims(&jwt);

        assert_eq!(account_id, None);
        assert_eq!(label.as_deref(), Some("user_123"));
    }

    #[test]
    fn handles_invalid_jwt_gracefully() {
        assert_eq!(extract_openai_claims("invalid.token"), (None, None));
        assert_eq!(extract_openai_claims("not-even-jwt"), (None, None));
    }
}

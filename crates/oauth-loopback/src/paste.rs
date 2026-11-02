use crate::server::LoopbackError;

/// Parse authorization code from a pasted string in paste fallback mode.
///
/// Accepts:
/// - `code#state` (standard Anthropic paste callback format)
/// - Full URL containing `code` and optional `state` (in query or fragment)
/// - Plain `code` string
///
/// If a state is present in the pasted string and `expected_state` is provided,
/// validates that they match. Returns `LoopbackError::StateMismatch` on mismatch.
pub fn parse_paste_code(
    pasted: &str,
    expected_state: Option<&str>,
) -> Result<String, LoopbackError> {
    let trimmed = pasted.trim();
    if trimmed.is_empty() {
        return Err(LoopbackError::MissingCode);
    }

    // Case 1: URL with query or fragment (e.g. https://platform.claude.com/oauth/code/callback?code=abc&state=xyz)
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        let (code, state) = extract_code_and_state_from_url(trimmed)?;
        if let (Some(extracted_state), Some(expected)) = (state, expected_state)
            && extracted_state != expected
        {
            return Err(LoopbackError::StateMismatch);
        }
        return Ok(code);
    }

    // Case 2: `code#state` format
    if let Some((code, state)) = trimmed.split_once('#') {
        let code = code.trim();
        let state = state.trim();

        // Could be fragment query style `#code=...&state=...`
        if code.is_empty() && (state.contains("code=") || state.contains("state=")) {
            let (c, s) = extract_params_from_query(state)?;
            if let (Some(extracted_state), Some(expected)) = (s, expected_state)
                && extracted_state != expected
            {
                return Err(LoopbackError::StateMismatch);
            }
            return Ok(c);
        }

        if code.is_empty() {
            return Err(LoopbackError::MissingCode);
        }

        if let Some(expected) = expected_state
            && state != expected
        {
            return Err(LoopbackError::StateMismatch);
        }
        return Ok(code.to_string());
    }

    // Case 3: Plain code
    Ok(trimmed.to_string())
}

fn extract_code_and_state_from_url(url: &str) -> Result<(String, Option<String>), LoopbackError> {
    // Prefer query (?...) over fragment (#...) for authorization code and state
    let query_part = url
        .split_once('?')
        .map(|(_, after_q)| after_q.split_once('#').map(|(q, _)| q).unwrap_or(after_q));
    let frag_part = url.split_once('#').map(|(_, f)| f);

    if let Some(query) = query_part
        && let Ok((code, state)) = extract_params_from_query(query)
    {
        let state = state.or_else(|| {
            frag_part.and_then(|f| extract_params_from_query(f).ok().and_then(|(_, s)| s))
        });
        return Ok((code, state));
    }

    if let Some(frag) = frag_part {
        return extract_params_from_query(frag);
    }

    Err(LoopbackError::MissingCode)
}

fn extract_params_from_query(query: &str) -> Result<(String, Option<String>), LoopbackError> {
    let mut code = None;
    let mut state = None;

    for part in query.split('&') {
        let (k, v) = part.split_once('=').unwrap_or((part, ""));
        let v = urlencoding::decode(v)
            .map(|c| c.into_owned())
            .unwrap_or_else(|_| v.to_string());
        match k {
            "code" => code = Some(v),
            "state" => state = Some(v),
            _ => {}
        }
    }

    let code = code.ok_or(LoopbackError::MissingCode)?;
    if code.trim().is_empty() {
        return Err(LoopbackError::MissingCode);
    }
    Ok((code, state))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_code_hash_state() {
        let input = "auth_code_123#expected_state_abc";
        let code = parse_paste_code(input, Some("expected_state_abc")).unwrap();
        assert_eq!(code, "auth_code_123");
    }

    #[test]
    fn rejects_code_hash_state_mismatch() {
        let input = "auth_code_123#wrong_state";
        let res = parse_paste_code(input, Some("expected_state_abc"));
        assert!(matches!(res, Err(LoopbackError::StateMismatch)));
    }

    #[test]
    fn parses_url_with_query_params() {
        let input = "https://platform.claude.com/oauth/code/callback?code=the_code&state=the_state";
        let code = parse_paste_code(input, Some("the_state")).unwrap();
        assert_eq!(code, "the_code");

        let err = parse_paste_code(input, Some("other_state"));
        assert!(matches!(err, Err(LoopbackError::StateMismatch)));
    }

    #[test]
    fn parses_bare_code() {
        let input = "bare_auth_code_xyz";
        let code = parse_paste_code(input, Some("some_state")).unwrap();
        assert_eq!(code, "bare_auth_code_xyz");
    }

    #[test]
    fn rejects_empty_input() {
        assert!(matches!(
            parse_paste_code("", None),
            Err(LoopbackError::MissingCode)
        ));
        assert!(matches!(
            parse_paste_code("   ", None),
            Err(LoopbackError::MissingCode)
        ));
    }

    #[test]
    fn url_with_query_and_fragment_prefers_query() {
        // Claude-style trailing fragment `#_=_` must not discard query params
        let input = "https://platform.claude.com/oauth/code/callback?code=REAL_CODE&state=s123#_=_";
        let code = parse_paste_code(input, Some("s123")).unwrap();
        assert_eq!(code, "REAL_CODE");

        // Query takes precedence over fragment when both contain code
        let malicious = "https://auth.example/callback?code=legit_code#code=evil_code";
        let code = parse_paste_code(malicious, None).unwrap();
        assert_eq!(code, "legit_code");
    }

    #[test]
    fn parses_fragment_only_url() {
        let input = "https://example.com/oauth#code=frag_code&state=frag_state";
        let code = parse_paste_code(input, Some("frag_state")).unwrap();
        assert_eq!(code, "frag_code");
    }

    #[test]
    fn rejects_empty_code_in_hash_format() {
        assert!(matches!(
            parse_paste_code("#", None),
            Err(LoopbackError::MissingCode)
        ));
        assert!(matches!(
            parse_paste_code("  #  ", None),
            Err(LoopbackError::MissingCode)
        ));
        assert!(matches!(
            parse_paste_code("#state=foo", None),
            Err(LoopbackError::MissingCode)
        ));
    }
}

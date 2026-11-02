/// Outcome of parsing one HTTP request that hit the loopback listener.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RedirectOutcome {
    /// `?code=...&state=...` — the happy path.
    Code { code: String, state: String },
    /// `?error=access_denied` etc.
    Error(String),
    /// `?code=...` but missing or empty `state` parameter.
    MissingState,
    /// Not the OAuth redirect (e.g. /favicon.ico) — keep listening.
    Unrelated,
}

/// Parse the request line of an HTTP request ("GET /...?... HTTP/1.1").
pub fn parse_redirect_request(request_head: &str) -> RedirectOutcome {
    let Some(first_line) = request_head.lines().next() else {
        return RedirectOutcome::Unrelated;
    };
    let mut parts = first_line.split_whitespace();
    let (Some(_method), Some(target)) = (parts.next(), parts.next()) else {
        return RedirectOutcome::Unrelated;
    };

    let Some((_path, query)) = target.split_once('?') else {
        return RedirectOutcome::Unrelated;
    };

    let mut code = None;
    let mut state = None;
    let mut error = None;
    for pair in query.split('&') {
        let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
        let v = urlencoding::decode(v)
            .map(|c| c.into_owned())
            .unwrap_or_else(|_| v.to_string());
        match k {
            "code" => code = Some(v),
            "state" => state = Some(v),
            "error" => error = Some(v),
            _ => {}
        }
    }

    if let Some(error) = error {
        return RedirectOutcome::Error(error);
    }
    match (code, state) {
        (Some(code), Some(state)) => {
            if code.trim().is_empty() {
                RedirectOutcome::Unrelated
            } else if state.trim().is_empty() {
                RedirectOutcome::MissingState
            } else {
                RedirectOutcome::Code { code, state }
            }
        }
        (Some(code), None) => {
            if code.trim().is_empty() {
                RedirectOutcome::Unrelated
            } else {
                RedirectOutcome::MissingState
            }
        }
        _ => RedirectOutcome::Unrelated,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_anthropic_callback_path() {
        let head = "GET /callback?code=ant_code_123&state=ant_state_456 HTTP/1.1\r\nHost: localhost:53692\r\n\r\n";
        assert_eq!(
            parse_redirect_request(head),
            RedirectOutcome::Code {
                code: "ant_code_123".to_string(),
                state: "ant_state_456".to_string(),
            }
        );
    }

    #[test]
    fn parses_openai_codex_callback_path() {
        let head = "GET /auth/callback?code=oai_code_abc&state=oai_state_def HTTP/1.1\r\nHost: localhost:1455\r\n\r\n";
        assert_eq!(
            parse_redirect_request(head),
            RedirectOutcome::Code {
                code: "oai_code_abc".to_string(),
                state: "oai_state_def".to_string(),
            }
        );
    }

    #[test]
    fn parses_gemini_google_callback_path() {
        let head =
            "GET /?code=gem_code_xyz&state=gem_state_uvw HTTP/1.1\r\nHost: 127.0.0.1:45678\r\n\r\n";
        assert_eq!(
            parse_redirect_request(head),
            RedirectOutcome::Code {
                code: "gem_code_xyz".to_string(),
                state: "gem_state_uvw".to_string(),
            }
        );
    }

    #[test]
    fn parses_error_redirect() {
        let head =
            "GET /auth/callback?error=access_denied&state=xyz HTTP/1.1\r\nHost: localhost:1455\r\n";
        assert_eq!(
            parse_redirect_request(head),
            RedirectOutcome::Error("access_denied".to_string())
        );
    }

    #[test]
    fn parses_unrelated_requests() {
        assert_eq!(
            parse_redirect_request("GET /favicon.ico HTTP/1.1\r\nHost: localhost:1455\r\n"),
            RedirectOutcome::Unrelated
        );
        assert_eq!(parse_redirect_request(""), RedirectOutcome::Unrelated);
        assert_eq!(
            parse_redirect_request("INVALID"),
            RedirectOutcome::Unrelated
        );
    }

    #[test]
    fn parses_code_without_state_as_missing_state() {
        let head = "GET /callback?code=some_code HTTP/1.1\r\nHost: localhost:53692\r\n\r\n";
        assert_eq!(parse_redirect_request(head), RedirectOutcome::MissingState);

        let empty_state =
            "GET /callback?code=some_code&state= HTTP/1.1\r\nHost: localhost:53692\r\n\r\n";
        assert_eq!(
            parse_redirect_request(empty_state),
            RedirectOutcome::MissingState
        );
    }
}

//! "Sign in with OpenRouter": the official OAuth PKCE flow for apps.
//! <https://openrouter.ai/docs/guides/overview/auth/oauth>
//!
//! Unlike the unofficial providers this one is not catalog-gated and yields a
//! plain API key (not a token pair), which the caller stores wherever the
//! manually-entered OpenRouter key lives.

use std::time::Duration;

use hypr_oauth_loopback::{LoopbackBindResult, LoopbackServer, generate_pkce};

use crate::error::Error;

pub const AUTH_URL: &str = "https://openrouter.ai/auth";
pub const KEYS_URL: &str = "https://openrouter.ai/api/v1/auth/keys";
pub const CALLBACK_PATH: &str = "/callback";
pub const REDIRECT_TIMEOUT: Duration = Duration::from_secs(300);
const EXCHANGE_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Clone)]
pub struct Endpoints {
    pub auth_url: String,
    pub keys_url: String,
}

impl Default for Endpoints {
    fn default() -> Self {
        Self {
            auth_url: AUTH_URL.to_string(),
            keys_url: KEYS_URL.to_string(),
        }
    }
}

pub fn build_auth_url(auth_url: &str, callback_url: &str, challenge: &str, state: &str) -> String {
    let params = [
        ("callback_url", callback_url),
        ("code_challenge", challenge),
        ("code_challenge_method", "S256"),
        ("state", state),
    ];
    let query = params
        .iter()
        .map(|(k, v)| format!("{}={}", k, urlencoding::encode(v)))
        .collect::<Vec<_>>()
        .join("&");
    format!("{auth_url}?{query}")
}

/// Exchanges the authorization code for a user-controlled API key.
pub async fn exchange_code(
    http: &reqwest::Client,
    keys_url: &str,
    code: &str,
    verifier: &str,
) -> Result<String, Error> {
    let resp = http
        .post(keys_url)
        .timeout(EXCHANGE_TIMEOUT)
        .json(&serde_json::json!({
            "code": code,
            "code_verifier": verifier,
            "code_challenge_method": "S256",
        }))
        .send()
        .await
        .map_err(|e| Error::TokenExchange(e.to_string()))?;

    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(Error::TokenExchange(format!(
            "OpenRouter returned {status}: {}",
            body.chars().take(300).collect::<String>()
        )));
    }

    let value: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| Error::TokenExchange(e.to_string()))?;
    value
        .get("key")
        .and_then(|k| k.as_str())
        .filter(|k| !k.is_empty())
        .map(str::to_string)
        .ok_or_else(|| Error::TokenExchange("response contained no key".to_string()))
}

/// Runs the whole flow: bind a loopback listener, hand the authorize URL to
/// `open`, wait for the redirect (or `cancel`), exchange the code for a key.
pub async fn sign_in<O, C>(
    http: &reqwest::Client,
    endpoints: &Endpoints,
    open: O,
    cancel: C,
    timeout: Duration,
) -> Result<String, Error>
where
    O: FnOnce(String) -> Result<(), String>,
    C: Future<Output = ()>,
{
    let pkce = generate_pkce();
    let state = uuid::Uuid::new_v4().simple().to_string();

    let server = match LoopbackServer::try_bind(0)
        .await
        .map_err(|e| Error::Loopback(e.to_string()))?
    {
        LoopbackBindResult::Bound(server) => server,
        LoopbackBindResult::PortInUse(port) => {
            return Err(Error::Loopback(format!("port {port} is in use")));
        }
    };
    let callback_url = format!("http://localhost:{}{CALLBACK_PATH}", server.port());
    let url = build_auth_url(&endpoints.auth_url, &callback_url, &pkce.challenge, &state);

    open(url).map_err(Error::OpenBrowser)?;

    let code = tokio::select! {
        redirect = server.wait_for_redirect(&state, timeout) => redirect?,
        () = cancel => return Err(Error::Cancelled),
    };

    exchange_code(http, &endpoints.keys_url, &code, &pkce.verifier).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use hypr_oauth_loopback::verify_pkce;
    use std::collections::HashMap;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn query(url: &str) -> HashMap<String, String> {
        let (_, q) = url.split_once('?').unwrap();
        q.split('&')
            .map(|p| {
                let (k, v) = p.split_once('=').unwrap();
                (k.to_string(), urlencoding::decode(v).unwrap().into_owned())
            })
            .collect()
    }

    #[test]
    fn auth_url_carries_callback_s256_challenge_and_state() {
        let url = build_auth_url(AUTH_URL, "http://localhost:5555/callback", "chal", "st");
        assert!(url.starts_with("https://openrouter.ai/auth?"));
        let q = query(&url);
        assert_eq!(q["callback_url"], "http://localhost:5555/callback");
        assert_eq!(q["code_challenge"], "chal");
        assert_eq!(q["code_challenge_method"], "S256");
        assert_eq!(q["state"], "st");
    }

    async fn keys_server(response: ResponseTemplate) -> MockServer {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/v1/auth/keys"))
            .respond_with(response)
            .mount(&server)
            .await;
        server
    }

    #[tokio::test]
    async fn exchange_posts_code_verifier_and_method_and_returns_key() {
        let server = keys_server(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({"key": "sk-or-v1-abc"})),
        )
        .await;
        let url = format!("{}/api/v1/auth/keys", server.uri());

        let key = exchange_code(&reqwest::Client::new(), &url, "the_code", "the_verifier")
            .await
            .unwrap();
        assert_eq!(key, "sk-or-v1-abc");

        let requests = server.received_requests().await.unwrap();
        assert_eq!(requests.len(), 1);
        let body: serde_json::Value = serde_json::from_slice(&requests[0].body).unwrap();
        assert_eq!(
            body,
            serde_json::json!({
                "code": "the_code",
                "code_verifier": "the_verifier",
                "code_challenge_method": "S256",
            })
        );
    }

    #[tokio::test]
    async fn exchange_surfaces_vendor_rejection() {
        let server = keys_server(
            ResponseTemplate::new(403).set_body_string("Invalid code or code_verifier"),
        )
        .await;
        let url = format!("{}/api/v1/auth/keys", server.uri());

        let err = exchange_code(&reqwest::Client::new(), &url, "c", "v")
            .await
            .unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("403"), "{msg}");
        assert!(msg.contains("Invalid code or code_verifier"), "{msg}");
    }

    #[tokio::test]
    async fn exchange_rejects_response_without_key() {
        let server =
            keys_server(ResponseTemplate::new(200).set_body_json(serde_json::json!({"key": ""})))
                .await;
        let url = format!("{}/api/v1/auth/keys", server.uri());

        let err = exchange_code(&reqwest::Client::new(), &url, "c", "v")
            .await
            .unwrap_err();
        assert!(matches!(err, Error::TokenExchange(_)));
    }

    /// Plays the browser: follows the authorize URL's callback_url with the
    /// echoed state, as OpenRouter does after the user approves.
    fn fake_browser(
        code: &'static str,
        state_override: Option<&'static str>,
        seen: std::sync::Arc<std::sync::Mutex<Option<String>>>,
    ) -> impl FnOnce(String) -> Result<(), String> {
        move |url| {
            *seen.lock().unwrap() = Some(url.clone());
            let q = query(&url);
            let callback = q["callback_url"].clone();
            let state = state_override
                .map(str::to_string)
                .unwrap_or(q["state"].clone());
            tokio::spawn(async move {
                let _ = reqwest::get(format!("{callback}?code={code}&state={state}")).await;
            });
            Ok(())
        }
    }

    #[tokio::test]
    async fn sign_in_runs_pkce_loopback_and_exchange_end_to_end() {
        let server = keys_server(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({"key": "sk-or-v1-e2e"})),
        )
        .await;
        let endpoints = Endpoints {
            auth_url: format!("{}/auth", server.uri()),
            keys_url: format!("{}/api/v1/auth/keys", server.uri()),
        };
        let seen = Default::default();

        let key = sign_in(
            &reqwest::Client::new(),
            &endpoints,
            fake_browser("browser_code", None, std::sync::Arc::clone(&seen)),
            std::future::pending(),
            Duration::from_secs(10),
        )
        .await
        .unwrap();
        assert_eq!(key, "sk-or-v1-e2e");

        let auth_url = seen.lock().unwrap().clone().unwrap();
        assert!(auth_url.starts_with(&format!("{}/auth?", server.uri())));
        let q = query(&auth_url);
        assert!(q["callback_url"].starts_with("http://localhost:"));
        assert!(q["callback_url"].ends_with("/callback"));

        let requests = server.received_requests().await.unwrap();
        let body: serde_json::Value = serde_json::from_slice(&requests[0].body).unwrap();
        assert_eq!(body["code"], "browser_code");
        assert!(
            verify_pkce(
                body["code_verifier"].as_str().unwrap(),
                &q["code_challenge"]
            ),
            "verifier sent to the key endpoint must hash to the challenge in the authorize URL"
        );
    }

    #[tokio::test]
    async fn sign_in_rejects_state_mismatch_without_exchanging() {
        let server = keys_server(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({"key": "never"})),
        )
        .await;
        let endpoints = Endpoints {
            auth_url: format!("{}/auth", server.uri()),
            keys_url: format!("{}/api/v1/auth/keys", server.uri()),
        };

        let err = sign_in(
            &reqwest::Client::new(),
            &endpoints,
            fake_browser("c", Some("forged"), Default::default()),
            std::future::pending(),
            Duration::from_secs(10),
        )
        .await
        .unwrap_err();
        assert!(matches!(err, Error::StateMismatch), "{err:?}");
        assert!(server.received_requests().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn sign_in_can_be_cancelled_while_waiting_for_the_browser() {
        let (tx, rx) = tokio::sync::oneshot::channel::<()>();
        tx.send(()).unwrap();

        let err = sign_in(
            &reqwest::Client::new(),
            &Endpoints::default(),
            |_url| Ok(()),
            async {
                let _ = rx.await;
            },
            Duration::from_secs(10),
        )
        .await
        .unwrap_err();
        assert!(matches!(err, Error::Cancelled), "{err:?}");
    }

    #[tokio::test]
    async fn sign_in_reports_browser_launch_failure() {
        let err = sign_in(
            &reqwest::Client::new(),
            &Endpoints::default(),
            |_url| Err("no browser".to_string()),
            std::future::pending(),
            Duration::from_secs(10),
        )
        .await
        .unwrap_err();
        assert!(
            matches!(err, Error::OpenBrowser(ref m) if m == "no browser"),
            "{err:?}"
        );
    }
}

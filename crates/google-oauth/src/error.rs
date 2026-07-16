#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid client json: {0}")]
    InvalidClientJson(String),
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("token endpoint returned {status}: {body}")]
    TokenEndpoint {
        status: reqwest::StatusCode,
        body: String,
    },
    #[error("failed to parse token response: {0}")]
    TokenParse(#[from] serde_json::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("authorization was denied: {0}")]
    AuthorizationDenied(String),
    #[error("redirect state mismatch (possible CSRF)")]
    StateMismatch,
    #[error("timed out waiting for the browser redirect")]
    RedirectTimeout,
    #[error("failed to open browser: {0}")]
    OpenBrowser(String),
    #[error("no refresh token in token response; re-run consent with prompt=consent")]
    MissingRefreshToken,
}

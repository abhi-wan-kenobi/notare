use crate::types::ProviderId;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("provider {0:?} is disabled in the catalog")]
    ProviderDisabled(ProviderId),

    #[error("unknown provider: {0}")]
    UnknownProvider(String),

    #[error("no active authentication flow for {0:?}")]
    NoActiveFlow(ProviderId),

    #[error("oauth state mismatch")]
    StateMismatch,

    #[error("oauth PKCE mismatch")]
    PkceMismatch,

    #[error("missing authorization code")]
    MissingCode,

    #[error("not signed in to {0:?}")]
    NotSignedIn(ProviderId),

    #[error("http error: {0}")]
    Http(String),

    #[error("token exchange error: {0}")]
    TokenExchange(String),

    #[error("onboarding error: {0}")]
    Onboarding(String),

    #[error("token store error: {0}")]
    Store(String),

    #[error("loopback error: {0}")]
    Loopback(String),

    #[error("could not open the browser: {0}")]
    OpenBrowser(String),

    #[error("sign-in cancelled")]
    Cancelled,

    #[error("serialization error: {0}")]
    Serialization(String),
}

impl From<reqwest::Error> for Error {
    fn from(e: reqwest::Error) -> Self {
        Self::Http(e.to_string())
    }
}

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Self::Serialization(e.to_string())
    }
}

impl From<hypr_oauth_loopback::LoopbackError> for Error {
    fn from(e: hypr_oauth_loopback::LoopbackError) -> Self {
        match e {
            hypr_oauth_loopback::LoopbackError::StateMismatch => Self::StateMismatch,
            hypr_oauth_loopback::LoopbackError::MissingCode => Self::MissingCode,
            other => Self::Loopback(other.to_string()),
        }
    }
}

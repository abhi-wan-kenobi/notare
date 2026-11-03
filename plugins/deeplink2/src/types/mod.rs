mod auth_callback;
mod billing_refresh;
mod integration_callback;
mod pair;

pub use auth_callback::*;
pub use billing_refresh::*;
pub use integration_callback::*;
pub use pair::*;

use serde::{Deserialize, Serialize};
use specta::Type;
use std::str::FromStr;

#[derive(Debug, Clone, serde::Serialize, specta::Type, tauri_specta::Event)]
pub struct DeepLinkEvent(pub DeepLink);

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(tag = "to", content = "search")]
pub enum DeepLink {
    #[serde(rename = "/auth/callback")]
    AuthCallback(AuthCallbackSearch),
    #[serde(rename = "/billing/refresh")]
    BillingRefresh(BillingRefreshSearch),
    #[serde(rename = "/integration/callback")]
    IntegrationCallback(IntegrationCallbackSearch),
    #[serde(rename = "/pair")]
    Pair(PairSearch),
}

impl DeepLink {
    pub fn path(&self) -> &'static str {
        match self {
            DeepLink::AuthCallback(_) => "/auth/callback",
            DeepLink::BillingRefresh(_) => "/billing/refresh",
            DeepLink::IntegrationCallback(_) => "/integration/callback",
            DeepLink::Pair(_) => "/pair",
        }
    }
}

impl FromStr for DeepLink {
    type Err = crate::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parsed = url::Url::parse(s)?;

        let host = parsed.host_str().unwrap_or("");
        let path = parsed.path().trim_start_matches('/');
        let full_path = if path.is_empty() {
            host.to_string()
        } else {
            format!("{}/{}", host, path)
        };

        let query = parsed.query().unwrap_or("");

        if full_path == "pair" {
            return parse_pair(&parsed);
        }

        match full_path.as_str() {
            "auth/callback" => Ok(DeepLink::AuthCallback(serde_qs::from_str(query)?)),
            "billing/refresh" => Ok(DeepLink::BillingRefresh(serde_qs::from_str(query)?)),
            "integration/callback" => Ok(DeepLink::IntegrationCallback(serde_qs::from_str(query)?)),
            _ => Err(crate::Error::UnknownPath(full_path)),
        }
    }
}

const PAIR_SCHEME: &str = "notare";

/// The offer lives in the fragment so it stays client-side; only the scheme
/// the app registers is accepted, and no error here echoes the payload.
fn parse_pair(parsed: &url::Url) -> Result<DeepLink, crate::Error> {
    if parsed.scheme() != PAIR_SCHEME {
        return Err(crate::Error::InvalidUrl(format!(
            "pair links must use the {PAIR_SCHEME}:// scheme"
        )));
    }

    match parsed.fragment() {
        Some(offer) if !offer.is_empty() => Ok(DeepLink::Pair(PairSearch {
            link: format!("{PAIR_SCHEME}://pair#{offer}"),
        })),
        _ => Err(crate::Error::InvalidUrl(
            "pair link is missing its offer fragment".to_string(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pair_link(link: &str) -> Option<String> {
        match DeepLink::from_str(link) {
            Ok(DeepLink::Pair(search)) => Some(search.link),
            _ => None,
        }
    }

    #[test]
    fn pair_link_keeps_the_fragment_payload_verbatim() {
        let link = "notare://pair#eyJ2IjoxLCJuIjoiQSJ9-_x";
        assert_eq!(pair_link(link).as_deref(), Some(link));
    }

    #[test]
    fn pair_link_without_a_fragment_is_rejected() {
        for link in [
            "notare://pair",
            "notare://pair/",
            "notare://pair#",
            "notare://pair?a=b",
        ] {
            assert!(
                matches!(DeepLink::from_str(link), Err(crate::Error::InvalidUrl(_))),
                "{link} should be rejected"
            );
        }
    }

    #[test]
    fn pair_link_with_extra_path_is_unknown() {
        assert!(matches!(
            DeepLink::from_str("notare://pair/extra#abc"),
            Err(crate::Error::UnknownPath(_))
        ));
    }

    #[test]
    fn pair_host_on_another_scheme_is_not_a_pair_link() {
        for link in [
            "https://pair#abc",
            "local://pair#abc",
            "notare-dev://pair#abc",
        ] {
            assert!(
                matches!(DeepLink::from_str(link), Err(crate::Error::InvalidUrl(_))),
                "{link} must not become a Pair link"
            );
        }
    }

    #[test]
    fn pair_errors_and_debug_never_leak_the_offer() {
        let secret = "SECRETOFFER123";
        let err = DeepLink::from_str(&format!("https://pair#{secret}")).unwrap_err();
        assert!(!err.to_string().contains(secret));

        let parsed = DeepLink::from_str(&format!("notare://pair#{secret}")).unwrap();
        assert!(!format!("{parsed:?}").contains(secret));
    }

    #[test]
    fn pair_serializes_with_the_pair_route() {
        let parsed = DeepLink::from_str("notare://pair#abc").unwrap();
        assert_eq!(parsed.path(), "/pair");
        assert_eq!(
            serde_json::to_value(&parsed).unwrap(),
            serde_json::json!({ "to": "/pair", "search": { "link": "notare://pair#abc" } })
        );
    }

    #[test]
    fn oauth_variants_still_parse() {
        assert!(matches!(
            DeepLink::from_str("notare://auth/callback?access_token=a&refresh_token=b"),
            Ok(DeepLink::AuthCallback(s)) if s.access_token == "a" && s.refresh_token == "b"
        ));
        assert!(matches!(
            DeepLink::from_str("notare://billing/refresh"),
            Ok(DeepLink::BillingRefresh(_))
        ));
        assert!(matches!(
            DeepLink::from_str(
                "notare://integration/callback?integration_id=x&status=success&return_to=todo"
            ),
            Ok(DeepLink::IntegrationCallback(s)) if s.status == "success" && s.return_to.as_deref() == Some("todo")
        ));
        assert!(matches!(
            DeepLink::from_str("notare://nonsense"),
            Err(crate::Error::UnknownPath(_))
        ));
    }
}

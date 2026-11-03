//! Unofficial OAuth client adapters and token management for LLM providers:
//! Anthropic (Claude Code), OpenAI (Codex), and Google (Gemini).
//!
//! Features:
//! - RFC 7636 PKCE & loopback HTTP redirect handling with paste fallback
//! - Automatic refresh 5 minutes before expiry
//! - OpenAI JWT claim extraction (chatgpt_account_id)
//! - Google Cloud Code Gemini lazy onboarding (loadCodeAssist -> onboardUser)
//! - Safe risk posture: OFF by default and gated by catalog

pub mod adapters;
pub mod catalog;
pub mod clock;
pub mod error;
pub mod jwt;
pub mod manager;
pub mod store;
pub mod types;

pub use catalog::{CatalogManager, DEFAULT_CATALOG_JSON, default_catalog, resolve_catalog};
pub use clock::{Clock, MockClock, SystemClock, should_refresh};
pub use error::Error;
pub use jwt::extract_openai_claims;
pub use manager::LlmAuthManager;
pub use store::{KEYRING_SCOPE, MemoryTokenStore, StoredTokens, TokenStore};
pub use types::*;

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, Utc};
    use std::sync::Arc;

    #[tokio::test]
    async fn start_refuses_disabled_provider() {
        let store = Arc::new(MemoryTokenStore::new());
        // Point the remote override at a closed loopback port so this test
        // checks the embedded default (all disabled) instead of whatever
        // catalog/llm-auth.json on `main` says today.
        let manager = LlmAuthManager::with_clock_and_catalog(
            store,
            Arc::new(clock::SystemClock),
            catalog::CatalogManager::with_override_url("http://127.0.0.1:9/catalog.json".into()),
            reqwest::Client::new(),
        );

        // All providers are disabled in the embedded catalog
        let res = manager.start(ProviderId::Anthropic).await;
        assert!(matches!(
            res,
            Err(Error::ProviderDisabled(ProviderId::Anthropic))
        ));

        let res_oai = manager.start(ProviderId::OpenaiCodex).await;
        assert!(matches!(
            res_oai,
            Err(Error::ProviderDisabled(ProviderId::OpenaiCodex))
        ));

        let res_gem = manager.start(ProviderId::Gemini).await;
        assert!(matches!(
            res_gem,
            Err(Error::ProviderDisabled(ProviderId::Gemini))
        ));
    }

    #[tokio::test]
    async fn token_expiry_refresh_with_fake_clock() {
        let base = Utc::now();
        let clock = Arc::new(MockClock::new(base));
        let store = Arc::new(MemoryTokenStore::new());

        let initial_tokens = StoredTokens {
            access_token: "at_valid".to_string(),
            refresh_token: Some("rt_test".to_string()),
            // Expires in 10 minutes
            expires_at: Some(base + Duration::minutes(10)),
            account_label: Some("user@test.org".to_string()),
            extra: std::collections::HashMap::new(),
        };

        store
            .set(ProviderId::Anthropic, &initial_tokens)
            .await
            .unwrap();

        let manager = LlmAuthManager::with_clock_and_catalog(
            store.clone(),
            clock.clone(),
            CatalogManager::new(),
            reqwest::Client::new(),
        );

        // Query status
        let status = manager.status(ProviderId::Anthropic).await.unwrap();
        assert!(status.signed_in);
        assert_eq!(status.account_label.as_deref(), Some("user@test.org"));

        // Access token when plenty of time remains
        let at_res = manager.access_token(ProviderId::Anthropic).await.unwrap();
        assert_eq!(at_res.token, "at_valid");

        // Advance fake clock by 6 minutes: 4 minutes remaining (< 5m threshold)
        clock.advance(Duration::minutes(6));

        // It detects refresh is needed. (Without mock network, refresh_token call fails with Http/TokenExchange error)
        let refresh_attempt = manager.access_token(ProviderId::Anthropic).await;
        assert!(refresh_attempt.is_err());
    }

    #[tokio::test]
    async fn logout_clears_tokens() {
        let store = Arc::new(MemoryTokenStore::new());
        let manager = LlmAuthManager::new(store.clone());

        let tokens = StoredTokens {
            access_token: "at_test".to_string(),
            refresh_token: None,
            expires_at: None,
            account_label: None,
            extra: std::collections::HashMap::new(),
        };

        store.set(ProviderId::Anthropic, &tokens).await.unwrap();
        assert!(
            manager
                .status(ProviderId::Anthropic)
                .await
                .unwrap()
                .signed_in
        );

        manager.logout(ProviderId::Anthropic).await.unwrap();
        assert!(
            !manager
                .status(ProviderId::Anthropic)
                .await
                .unwrap()
                .signed_in
        );
    }
}

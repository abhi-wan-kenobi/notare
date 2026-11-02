use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::error::Error;
use crate::types::ProviderId;

/// Stored OAuth tokens and metadata held securely in Rust.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct StoredTokens {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(default)]
    pub account_label: Option<String>,
    #[serde(default)]
    pub extra: HashMap<String, String>,
}

/// Scope name used when persisting tokens in the secure store / keyring.
pub const KEYRING_SCOPE: &str = "llm-auth";

/// Abstraction for storing OAuth tokens.
pub trait TokenStore: Send + Sync {
    fn get(
        &self,
        provider: ProviderId,
    ) -> impl Future<Output = Result<Option<StoredTokens>, Error>> + Send;

    fn set(
        &self,
        provider: ProviderId,
        tokens: &StoredTokens,
    ) -> impl Future<Output = Result<(), Error>> + Send;

    fn delete(&self, provider: ProviderId) -> impl Future<Output = Result<(), Error>> + Send;
}

/// In-memory token store for testing and headless environments.
#[derive(Debug, Clone, Default)]
pub struct MemoryTokenStore {
    tokens: Arc<RwLock<HashMap<ProviderId, StoredTokens>>>,
}

impl MemoryTokenStore {
    pub fn new() -> Self {
        Self::default()
    }
}

impl TokenStore for MemoryTokenStore {
    async fn get(&self, provider: ProviderId) -> Result<Option<StoredTokens>, Error> {
        let lock = self.tokens.read().await;
        Ok(lock.get(&provider).cloned())
    }

    async fn set(&self, provider: ProviderId, tokens: &StoredTokens) -> Result<(), Error> {
        let mut lock = self.tokens.write().await;
        lock.insert(provider, tokens.clone());
        Ok(())
    }

    async fn delete(&self, provider: ProviderId) -> Result<(), Error> {
        let mut lock = self.tokens.write().await;
        lock.remove(&provider);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, Utc};

    #[tokio::test]
    async fn token_store_round_trip_with_in_memory_store() {
        let store = MemoryTokenStore::new();

        // Initially empty
        assert_eq!(store.get(ProviderId::Anthropic).await.unwrap(), None);

        let mut extra = HashMap::new();
        extra.insert("test_key".to_string(), "test_val".to_string());

        let tokens = StoredTokens {
            access_token: "at_secret_123".to_string(),
            refresh_token: Some("rt_secret_456".to_string()),
            expires_at: Some(Utc::now() + Duration::hours(1)),
            account_label: Some("test@example.com".to_string()),
            extra,
        };

        // Store tokens
        store.set(ProviderId::Anthropic, &tokens).await.unwrap();

        // Read back
        let loaded = store
            .get(ProviderId::Anthropic)
            .await
            .unwrap()
            .expect("must be found");
        assert_eq!(loaded, tokens);

        // Update tokens (e.g. after refresh)
        let updated = StoredTokens {
            access_token: "at_refreshed_789".to_string(),
            ..loaded
        };
        store.set(ProviderId::Anthropic, &updated).await.unwrap();

        let loaded_updated = store
            .get(ProviderId::Anthropic)
            .await
            .unwrap()
            .expect("must be found");
        assert_eq!(loaded_updated.access_token, "at_refreshed_789");
        assert_eq!(
            loaded_updated.refresh_token,
            Some("rt_secret_456".to_string())
        );

        // Delete tokens
        store.delete(ProviderId::Anthropic).await.unwrap();
        assert_eq!(store.get(ProviderId::Anthropic).await.unwrap(), None);
    }
}

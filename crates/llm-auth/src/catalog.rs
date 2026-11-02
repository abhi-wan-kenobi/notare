use std::sync::Arc;
use std::time::Duration;
use tokio::sync::OnceCell;

use crate::types::{CatalogProvider, CatalogResult, ProviderId};

pub const DEFAULT_CATALOG_JSON: &str = r#"{
  "providers": [
    {
      "id": "anthropic",
      "enabled": false,
      "label": "Anthropic (Claude)",
      "beta_headers": {
        "anthropic-beta": "claude-code-20250219,oauth-2025-04-20"
      },
      "system_prefix": "You are Claude Code, Anthropic's official CLI for Claude.",
      "base_url": "https://api.anthropic.com/v1",
      "notes": "Unofficial Claude Code OAuth integration. Feature is OFF by default; the user's consumer account carries the risk."
    },
    {
      "id": "openai_codex",
      "enabled": false,
      "label": "OpenAI (ChatGPT Codex)",
      "beta_headers": {
        "OpenAI-Beta": "responses=experimental"
      },
      "system_prefix": null,
      "base_url": "https://chatgpt.com/backend-api/codex",
      "notes": "Unofficial Codex OAuth integration using ChatGPT Responses API. Feature is OFF by default; the user's consumer account carries the risk."
    },
    {
      "id": "gemini",
      "enabled": false,
      "label": "Google Gemini (Code Assist)",
      "beta_headers": {},
      "system_prefix": null,
      "base_url": "https://cloudcode-pa.googleapis.com/v1internal",
      "notes": "Unofficial Gemini CLI OAuth integration using Cloud Code Assist. Feature is OFF by default; the user's consumer account carries the risk."
    }
  ]
}"#;

pub const REMOTE_CATALOG_URL: &str =
    "https://raw.githubusercontent.com/abhi-wan-kenobi/notare/main/catalog/llm-auth.json";
pub const REMOTE_TIMEOUT: Duration = Duration::from_secs(10);

pub fn default_catalog() -> CatalogResult {
    parse_catalog(DEFAULT_CATALOG_JSON).expect("embedded catalog JSON must be valid")
}

pub fn parse_catalog(json: &str) -> Result<CatalogResult, serde_json::Error> {
    serde_json::from_str(json)
}

/// Resolves catalog with override precedence: remote overrides embedded,
/// but if remote is missing, invalid, or malformed, fallback to embedded.
pub fn resolve_catalog(embedded_json: &str, remote_body: Option<&str>) -> CatalogResult {
    if let Some(body) = remote_body
        && let Ok(catalog) = parse_catalog(body)
    {
        return catalog;
    }
    parse_catalog(embedded_json).expect("embedded default catalog must be valid")
}

/// Manages fetching the remote catalog at most once per launch with timeout.
#[derive(Clone)]
pub struct CatalogManager {
    cell: Arc<OnceCell<CatalogResult>>,
    http: reqwest::Client,
    override_url: Option<String>,
}

impl Default for CatalogManager {
    fn default() -> Self {
        Self {
            cell: Arc::new(OnceCell::new()),
            http: reqwest::Client::new(),
            override_url: None,
        }
    }
}

impl CatalogManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_override_url(url: String) -> Self {
        Self {
            cell: Arc::new(OnceCell::new()),
            http: reqwest::Client::new(),
            override_url: Some(url),
        }
    }

    /// Returns the resolved catalog, attempting remote fetch at most once.
    pub async fn get_catalog(&self) -> CatalogResult {
        self.cell
            .get_or_init(|| async {
                let url = self.override_url.as_deref().unwrap_or(REMOTE_CATALOG_URL);

                let remote_body = match self.http.get(url).timeout(REMOTE_TIMEOUT).send().await {
                    Ok(resp) if resp.status().is_success() => resp.text().await.ok(),
                    _ => None,
                };

                resolve_catalog(DEFAULT_CATALOG_JSON, remote_body.as_deref())
            })
            .await
            .clone()
    }

    pub async fn is_provider_enabled(&self, id: ProviderId) -> bool {
        let catalog = self.get_catalog().await;
        catalog
            .providers
            .iter()
            .find(|p| p.id == id)
            .map(|p| p.enabled)
            .unwrap_or(false)
    }

    pub async fn find_provider(&self, id: ProviderId) -> Option<CatalogProvider> {
        let catalog = self.get_catalog().await;
        catalog.providers.into_iter().find(|p| p.id == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_catalog_has_all_three_providers_disabled_by_default() {
        let catalog = default_catalog();
        assert_eq!(catalog.providers.len(), 3);
        for p in &catalog.providers {
            assert!(!p.enabled, "Provider {:?} must be OFF by default", p.id);
        }
    }

    #[test]
    fn catalog_override_precedence_valid_remote_overrides_embedded() {
        let remote_json = r#"{
            "providers": [
                {
                    "id": "anthropic",
                    "enabled": true,
                    "label": "Remote Anthropic",
                    "beta_headers": {},
                    "system_prefix": null,
                    "base_url": "https://api.anthropic.com/v1",
                    "notes": "Enabled via remote"
                }
            ]
        }"#;

        let resolved = resolve_catalog(DEFAULT_CATALOG_JSON, Some(remote_json));
        assert_eq!(resolved.providers.len(), 1);
        assert_eq!(resolved.providers[0].id, ProviderId::Anthropic);
        assert!(resolved.providers[0].enabled);
        assert_eq!(resolved.providers[0].label, "Remote Anthropic");
    }

    #[test]
    fn catalog_override_precedence_malformed_remote_ignored() {
        let malformed_json = "{ invalid_json_here: true ";
        let resolved = resolve_catalog(DEFAULT_CATALOG_JSON, Some(malformed_json));

        // Falls back to embedded default
        assert_eq!(resolved.providers.len(), 3);
        assert!(!resolved.providers[0].enabled);
    }

    #[test]
    fn catalog_override_precedence_none_remote_uses_embedded() {
        let resolved = resolve_catalog(DEFAULT_CATALOG_JSON, None);
        assert_eq!(resolved.providers.len(), 3);
        assert!(!resolved.providers[0].enabled);
    }
}

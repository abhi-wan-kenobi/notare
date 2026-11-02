use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

use hypr_oauth_loopback::{
    LoopbackBindResult, LoopbackError, LoopbackServer, PkcePair, generate_pkce, parse_paste_code,
};

use crate::adapters::{anthropic, gemini, openai_codex};
use crate::catalog::CatalogManager;
use crate::clock::{Clock, SystemClock, should_refresh};
use crate::error::Error;
use crate::store::TokenStore;
use crate::types::{
    AccessTokenResult, AuthMode, AuthStatus, CatalogResult, ProviderId, StartFlowResult,
};

struct ActiveFlow {
    #[allow(dead_code)]
    provider: ProviderId,
    state: String,
    pkce: PkcePair,
    redirect_uri: String,
    #[allow(dead_code)]
    mode: AuthMode,
    listener_rx: Option<tokio::sync::oneshot::Receiver<Result<String, LoopbackError>>>,
}

#[derive(Clone)]
pub struct LlmAuthManager<S: TokenStore, C: Clock = SystemClock> {
    store: Arc<S>,
    clock: Arc<C>,
    catalog: CatalogManager,
    http: reqwest::Client,
    active_flows: Arc<RwLock<HashMap<ProviderId, ActiveFlow>>>,
}

impl<S: TokenStore> LlmAuthManager<S, SystemClock> {
    pub fn new(store: Arc<S>) -> Self {
        Self {
            store,
            clock: Arc::new(SystemClock),
            catalog: CatalogManager::new(),
            http: reqwest::Client::new(),
            active_flows: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl<S: TokenStore, C: Clock> LlmAuthManager<S, C> {
    pub fn with_clock_and_catalog(
        store: Arc<S>,
        clock: Arc<C>,
        catalog: CatalogManager,
        http: reqwest::Client,
    ) -> Self {
        Self {
            store,
            clock,
            catalog,
            http,
            active_flows: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn store(&self) -> &S {
        &self.store
    }

    pub fn catalog_manager(&self) -> &CatalogManager {
        &self.catalog
    }

    /// Query current status for a provider.
    pub async fn status(&self, provider: ProviderId) -> Result<AuthStatus, Error> {
        let tokens = self.store.get(provider).await?;
        match tokens {
            Some(stored) => Ok(AuthStatus {
                signed_in: true,
                account_label: stored.account_label,
                expires_at: stored.expires_at.map(|t| t.to_rfc3339()),
            }),
            None => Ok(AuthStatus {
                signed_in: false,
                account_label: None,
                expires_at: None,
            }),
        }
    }

    /// Start OAuth flow. Checks catalog; refuses disabled providers.
    pub async fn start(&self, provider: ProviderId) -> Result<StartFlowResult, Error> {
        if !self.catalog.is_provider_enabled(provider).await {
            return Err(Error::ProviderDisabled(provider));
        }

        let pkce = generate_pkce();
        let state = uuid::Uuid::new_v4().simple().to_string();

        let (mode, redirect_uri, listener_rx) = match provider {
            ProviderId::Anthropic => {
                match LoopbackServer::try_bind(anthropic::LOOPBACK_PORT).await {
                    Ok(LoopbackBindResult::Bound(server)) => {
                        let rx = server.spawn(state.clone(), Duration::from_secs(300));
                        (
                            AuthMode::Loopback,
                            anthropic::LOOPBACK_REDIRECT_URI.to_string(),
                            Some(rx),
                        )
                    }
                    _ => (
                        AuthMode::Paste,
                        anthropic::PASTE_REDIRECT_URI.to_string(),
                        None,
                    ),
                }
            }
            ProviderId::OpenaiCodex => {
                match LoopbackServer::try_bind(openai_codex::LOOPBACK_PORT).await {
                    Ok(LoopbackBindResult::Bound(server)) => {
                        let rx = server.spawn(state.clone(), Duration::from_secs(300));
                        (
                            AuthMode::Loopback,
                            openai_codex::LOOPBACK_REDIRECT_URI.to_string(),
                            Some(rx),
                        )
                    }
                    _ => (
                        AuthMode::Paste,
                        openai_codex::LOOPBACK_REDIRECT_URI.to_string(),
                        None,
                    ),
                }
            }
            ProviderId::Gemini => match LoopbackServer::try_bind(0).await {
                Ok(LoopbackBindResult::Bound(server)) => {
                    let port = server.port();
                    let rx = server.spawn(state.clone(), Duration::from_secs(300));
                    (
                        AuthMode::Loopback,
                        format!("http://127.0.0.1:{port}/"),
                        Some(rx),
                    )
                }
                _ => (AuthMode::Paste, "http://127.0.0.1/".to_string(), None),
            },
        };

        let authorize_url = match provider {
            ProviderId::Anthropic => {
                anthropic::build_auth_url(&redirect_uri, &state, &pkce.challenge)
            }
            ProviderId::OpenaiCodex => {
                openai_codex::build_auth_url(&redirect_uri, &state, &pkce.challenge)
            }
            ProviderId::Gemini => gemini::build_auth_url(&redirect_uri, &state, &pkce.challenge),
        };

        let mut flows = self.active_flows.write().await;
        flows.insert(
            provider,
            ActiveFlow {
                provider,
                state,
                pkce,
                redirect_uri,
                mode,
                listener_rx,
            },
        );

        Ok(StartFlowResult {
            authorize_url,
            mode,
        })
    }

    /// Complete OAuth flow by either awaiting loopback callback or processing pasted code.
    pub async fn complete(
        &self,
        provider: ProviderId,
        pasted_code: Option<String>,
    ) -> Result<AuthStatus, Error> {
        let flow = {
            let mut flows = self.active_flows.write().await;
            flows
                .remove(&provider)
                .ok_or(Error::NoActiveFlow(provider))?
        };

        let code = match pasted_code.filter(|s| !s.trim().is_empty()) {
            Some(pasted) => parse_paste_code(&pasted, Some(&flow.state))?,
            None => {
                let rx = flow.listener_rx.ok_or(Error::MissingCode)?;
                rx.await
                    .map_err(|_| Error::Loopback("listener dropped".to_string()))??
            }
        };

        let tokens = match provider {
            ProviderId::Anthropic => {
                anthropic::exchange_code(
                    &self.http,
                    &code,
                    &flow.redirect_uri,
                    &flow.pkce.verifier,
                    &flow.state,
                )
                .await?
            }
            ProviderId::OpenaiCodex => {
                openai_codex::exchange_code(
                    &self.http,
                    &code,
                    &flow.redirect_uri,
                    &flow.pkce.verifier,
                )
                .await?
            }
            ProviderId::Gemini => {
                gemini::exchange_code(&self.http, &code, &flow.redirect_uri, &flow.pkce.verifier)
                    .await?
            }
        };

        self.store.set(provider, &tokens).await?;

        Ok(AuthStatus {
            signed_in: true,
            account_label: tokens.account_label,
            expires_at: tokens.expires_at.map(|t| t.to_rfc3339()),
        })
    }

    /// Cancel an in-flight OAuth flow.
    pub async fn cancel(&self, provider: ProviderId) -> Result<(), Error> {
        let mut flows = self.active_flows.write().await;
        flows.remove(&provider);
        Ok(())
    }

    /// Logout and delete stored tokens, revoking if vendor supports it.
    pub async fn logout(&self, provider: ProviderId) -> Result<(), Error> {
        let _ = self.cancel(provider).await;

        if let Ok(Some(tokens)) = self.store.get(provider).await
            && let Some(refresh_tok) = tokens.refresh_token.as_deref()
        {
            match provider {
                ProviderId::OpenaiCodex => {
                    let _ = openai_codex::revoke_token(&self.http, refresh_tok).await;
                }
                ProviderId::Gemini => {
                    let _ = gemini::revoke_token(&self.http, refresh_tok).await;
                }
                ProviderId::Anthropic => {}
            }
        }

        self.store.delete(provider).await?;
        Ok(())
    }

    /// Retrieve access token, refreshing if expiring within 5 minutes.
    /// Gemini executes lazy onboarding on first use to acquire `project_id`.
    pub async fn access_token(&self, provider: ProviderId) -> Result<AccessTokenResult, Error> {
        let mut tokens = self
            .store
            .get(provider)
            .await?
            .ok_or(Error::NotSignedIn(provider))?;

        if should_refresh(tokens.expires_at, self.clock.now()) {
            let refresh_tok = tokens.refresh_token.as_deref().ok_or_else(|| {
                Error::TokenExchange("no refresh token to refresh access token".to_string())
            })?;

            let refreshed = match provider {
                ProviderId::Anthropic => {
                    anthropic::refresh_token(&self.http, refresh_tok, &tokens).await?
                }
                ProviderId::OpenaiCodex => {
                    openai_codex::refresh_token(&self.http, refresh_tok, &tokens).await?
                }
                ProviderId::Gemini => {
                    gemini::refresh_token(&self.http, refresh_tok, &tokens).await?
                }
            };

            self.store.set(provider, &refreshed).await?;
            tokens = refreshed;
        }

        // Lazy Gemini onboarding
        if provider == ProviderId::Gemini && !tokens.extra.contains_key("project_id") {
            let project_id =
                gemini::ensure_onboarded(&self.http, &tokens.access_token, None).await?;
            tokens.extra.insert("project_id".to_string(), project_id);
            self.store.set(provider, &tokens).await?;
        }

        Ok(AccessTokenResult {
            token: tokens.access_token,
            extra: tokens.extra,
        })
    }

    /// Get catalog of LLM providers.
    pub async fn catalog(&self) -> Result<CatalogResult, Error> {
        Ok(self.catalog.get_catalog().await)
    }
}

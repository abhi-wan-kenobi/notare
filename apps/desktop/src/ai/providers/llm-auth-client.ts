import {
  type AccessTokenResult,
  type AuthStatus,
  type CatalogProvider,
  type CatalogResult,
  commands as pluginCommands,
  type ProviderId,
  type Result,
  type StartFlowResult,
} from "@hypr/plugin-llm-auth";

export type LlmAuthProvider = ProviderId;
export type LlmAuthStatus = AuthStatus;
export type LlmAuthStart = StartFlowResult;
export type LlmAuthToken = AccessTokenResult;
export type LlmAuthCatalogProvider = CatalogProvider;
export type LlmAuthCatalog = CatalogResult;
export type { Result };

export type SubscriptionProviderId =
  | "anthropic_subscription"
  | "openai_subscription"
  | "gemini_subscription";

export const SUBSCRIPTION_PROVIDER_MAP: Record<
  SubscriptionProviderId,
  LlmAuthProvider
> = {
  anthropic_subscription: "anthropic",
  openai_subscription: "openai_codex",
  gemini_subscription: "gemini",
};

export const REVERSE_PROVIDER_MAP: Record<
  LlmAuthProvider,
  SubscriptionProviderId
> = {
  anthropic: "anthropic_subscription",
  openai_codex: "openai_subscription",
  gemini: "gemini_subscription",
};

export function toPluginProvider(id: SubscriptionProviderId): LlmAuthProvider {
  return SUBSCRIPTION_PROVIDER_MAP[id];
}

export function toDesktopProvider(id: LlmAuthProvider): SubscriptionProviderId {
  return REVERSE_PROVIDER_MAP[id];
}

export const DEFAULT_CATALOG: LlmAuthCatalog = {
  providers: [
    {
      id: "anthropic",
      enabled: false,
      label: "Anthropic (Claude)",
      beta_headers: {
        "anthropic-beta": "claude-code-20250219,oauth-2025-04-20",
      },
      system_prefix:
        "You are Claude Code, Anthropic's official CLI for Claude.",
      base_url: "https://api.anthropic.com/v1",
      notes: "Unofficial subscription access via Claude Code OAuth",
    },
    {
      id: "openai_codex",
      enabled: false,
      label: "OpenAI Codex",
      beta_headers: {
        "OpenAI-Beta": "responses=experimental",
      },
      system_prefix: null,
      base_url: "https://chatgpt.com/backend-api/codex",
      notes: "Unofficial subscription access via ChatGPT Codex OAuth",
    },
    {
      id: "gemini",
      enabled: false,
      label: "Google Gemini",
      beta_headers: {},
      system_prefix: null,
      base_url: "https://cloudcode-pa.googleapis.com",
      notes: "Unofficial subscription access via Gemini CLI OAuth",
    },
  ],
};

export const commands = {
  status: (provider: LlmAuthProvider) => pluginCommands.status(provider),
  start: (provider: LlmAuthProvider) => pluginCommands.start(provider),
  complete: (provider: LlmAuthProvider, pastedCode: string | null) =>
    pluginCommands.complete(provider, pastedCode),
  cancel: (provider: LlmAuthProvider) => pluginCommands.cancel(provider),
  logout: (provider: LlmAuthProvider) => pluginCommands.logout(provider),
  accessToken: (provider: LlmAuthProvider) =>
    pluginCommands.accessToken(provider),
  catalog: () => pluginCommands.catalog(),
};

export async function unwrap<T>(
  promise: Promise<Result<T, string>>,
): Promise<T> {
  const res = await promise;
  if (res.status === "ok") {
    return res.data;
  }
  throw new Error(res.error);
}

/**
 * The catalog is the kill switch for the unofficial providers. If it cannot
 * be read (plugin missing, IPC failure) nothing is enabled — never fall back
 * to the embedded constants here, those exist only for header values.
 */
export async function fetchCatalog(): Promise<LlmAuthCatalog> {
  try {
    const res = await commands.catalog();
    return res.status === "ok" ? res.data : { providers: [] };
  } catch {
    return { providers: [] };
  }
}

export async function fetchAuthStatus(
  provider: LlmAuthProvider,
): Promise<LlmAuthStatus> {
  const res = await commands.status(provider);
  if (res.status === "ok") {
    return res.data;
  }
  return { signed_in: false, account_label: null, expires_at: null };
}

export async function fetchAccessToken(
  provider: LlmAuthProvider,
): Promise<LlmAuthToken> {
  return unwrap(commands.accessToken(provider));
}

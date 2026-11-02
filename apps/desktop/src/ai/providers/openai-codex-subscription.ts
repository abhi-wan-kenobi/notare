import { createOpenAI } from "@ai-sdk/openai";

import {
  DEFAULT_CATALOG,
  type LlmAuthCatalogProvider,
  type LlmAuthToken,
} from "./llm-auth-client";
import { createSubscriptionFetch } from "./subscription-fetch";

export const OPENAI_CODEX_SUBSCRIPTION_MODELS = [
  "gpt-5-codex",
  "gpt-5",
] as const;

export type OpenAICodexSubscriptionModelId =
  | (typeof OPENAI_CODEX_SUBSCRIPTION_MODELS)[number]
  | (string & {});

export type OpenAICodexSubscriptionOptions = {
  fetchFn?: typeof fetch;
  getToken?: () => Promise<LlmAuthToken>;
  catalog?: LlmAuthCatalogProvider;
};

export function createOpenAICodexSubscriptionFetch(
  options: OpenAICodexSubscriptionOptions = {},
): typeof fetch {
  const catalog =
    options.catalog ??
    DEFAULT_CATALOG.providers.find((p) => p.id === "openai_codex");
  const betaHeaders = catalog?.beta_headers ?? {
    "OpenAI-Beta": "responses=experimental",
  };

  return createSubscriptionFetch({
    provider: "openai_codex",
    fetchFn: options.fetchFn,
    getToken: options.getToken,
    injectHeaders: (token, extra) => {
      const headers: Record<string, string> = {
        Authorization: `Bearer ${token}`,
        ...betaHeaders,
      };
      if (extra?.chatgpt_account_id) {
        headers["chatgpt-account-id"] = extra.chatgpt_account_id;
      }
      return headers;
    },
  });
}

export function createOpenAICodexSubscriptionModel(
  modelId: string,
  options: OpenAICodexSubscriptionOptions = {},
) {
  const catalog =
    options.catalog ??
    DEFAULT_CATALOG.providers.find((p) => p.id === "openai_codex");
  const baseURL = catalog?.base_url ?? "https://chatgpt.com/backend-api/codex";
  const customFetch = createOpenAICodexSubscriptionFetch(options);

  const provider = createOpenAI({
    baseURL,
    fetch: customFetch,
    apiKey: "subscription",
  });

  return provider.responses(modelId);
}

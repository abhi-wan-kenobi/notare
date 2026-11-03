import { createAnthropic } from "@ai-sdk/anthropic";

import {
  DEFAULT_CATALOG,
  type LlmAuthCatalogProvider,
  type LlmAuthToken,
} from "./llm-auth-client";
import { createSubscriptionFetch } from "./subscription-fetch";

export const ANTHROPIC_SUBSCRIPTION_MODELS = [
  "claude-sonnet-4-5",
  "claude-opus-4-1",
  "claude-haiku-4-5",
] as const;

export type AnthropicSubscriptionModelId =
  | (typeof ANTHROPIC_SUBSCRIPTION_MODELS)[number]
  | (string & {});

export function transformAnthropicRequestBody(
  bodyText: string,
  systemPrefix: string | null,
): string {
  if (!systemPrefix) {
    return bodyText;
  }

  try {
    const body = JSON.parse(bodyText) as Record<string, unknown>;
    const prefixBlock = { type: "text", text: systemPrefix };

    if (!body.system) {
      body.system = [prefixBlock];
    } else if (typeof body.system === "string") {
      body.system = [prefixBlock, { type: "text", text: body.system }];
    } else if (Array.isArray(body.system)) {
      const first = body.system[0];
      let isAlreadyPrefix = false;
      if (typeof first === "object" && first !== null && "text" in first) {
        isAlreadyPrefix = first.text === systemPrefix;
      }
      if (!isAlreadyPrefix) {
        body.system = [prefixBlock, ...body.system];
      }
    }

    return JSON.stringify(body);
  } catch {
    return bodyText;
  }
}

export type AnthropicSubscriptionOptions = {
  fetchFn?: typeof fetch;
  getToken?: () => Promise<LlmAuthToken>;
  catalog?: LlmAuthCatalogProvider;
};

export function createAnthropicSubscriptionFetch(
  options: AnthropicSubscriptionOptions = {},
): typeof fetch {
  const catalog =
    options.catalog ??
    DEFAULT_CATALOG.providers.find((p) => p.id === "anthropic");
  const betaHeaders = catalog?.beta_headers ?? {
    "anthropic-beta": "claude-code-20250219,oauth-2025-04-20",
  };
  const systemPrefix =
    catalog?.system_prefix ??
    "You are Claude Code, Anthropic's official CLI for Claude.";

  return createSubscriptionFetch({
    provider: "anthropic",
    fetchFn: options.fetchFn,
    getToken: options.getToken,
    // OAuth requests must look like the official CLI's: bearer token only —
    // no browser-access flag, no x-api-key (the SDK inserts a placeholder).
    dropHeaders: ["x-api-key", "anthropic-dangerous-direct-browser-access"],
    injectHeaders: (token) => ({
      Authorization: `Bearer ${token}`,
      "anthropic-version": "2023-06-01",
      ...betaHeaders,
    }),
    transformBody: (bodyText) =>
      transformAnthropicRequestBody(bodyText, systemPrefix),
  });
}

export function createAnthropicSubscriptionModel(
  modelId: string,
  options: AnthropicSubscriptionOptions = {},
) {
  const catalog =
    options.catalog ??
    DEFAULT_CATALOG.providers.find((p) => p.id === "anthropic");
  const baseURL = catalog?.base_url ?? "https://api.anthropic.com/v1";
  const customFetch = createAnthropicSubscriptionFetch(options);

  const provider = createAnthropic({
    baseURL,
    fetch: customFetch,
    apiKey: "subscription",
  });

  return provider(modelId);
}

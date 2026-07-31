import { createAnthropic } from "@ai-sdk/anthropic";
import { createAzure } from "@ai-sdk/azure";
import { createGoogleGenerativeAI } from "@ai-sdk/google";
import { createOpenAI } from "@ai-sdk/openai";
import { createOpenAICompatible } from "@ai-sdk/openai-compatible";
import { createOpenRouter } from "@openrouter/ai-sdk-provider";
import { fetch as tauriFetch } from "@tauri-apps/plugin-http";
import { extractReasoningMiddleware, wrapLanguageModel } from "ai";
import { useMemo, useRef } from "react";

import type { CharTask } from "@hypr/api-client";
import type { AIProviderStorage } from "@hypr/store";

import { createAuthFetch } from "../auth-fetch";
import {
  isCloudProvider,
  type LlmScope,
  resolveScopeSelection,
  SCOPE_SETTING_KEYS,
} from "../scope";
import { createTracedFetch, tracedFetch } from "../traced-fetch";

import { useAuth } from "~/auth";
import { useBillingAccess } from "~/auth/billing";
import { env } from "~/env";
import { type ProviderId, PROVIDERS } from "~/settings/ai/llm/shared";
import {
  getProviderSelectionBlockers,
  type ProviderEligibilityContext,
} from "~/settings/ai/shared/eligibility";
import { useAiProvider } from "~/settings/providers";
import { useConfigValues } from "~/shared/config";

type LanguageModelV3 = Parameters<typeof wrapLanguageModel>[0]["model"];

type LLMConnectionInfo = {
  providerId: ProviderId;
  modelId: string;
  baseUrl: string;
  apiKey: string;
};

export type LLMConnectionStatus =
  | { status: "pending"; reason: "missing_provider" }
  | { status: "pending"; reason: "missing_model"; providerId: ProviderId }
  | { status: "error"; reason: "provider_not_found"; providerId: string }
  | { status: "error"; reason: "unauthenticated"; providerId: "hyprnote" }
  | { status: "error"; reason: "not_pro"; providerId: "hyprnote" }
  | {
      status: "error";
      reason: "missing_config";
      providerId: ProviderId;
      missing: Array<"base_url" | "api_key">;
    }
  | { status: "success"; providerId: ProviderId; isHosted: boolean };

type LLMConnectionResult = {
  conn: LLMConnectionInfo | null;
  status: LLMConnectionStatus;
};

/**
 * Trace-label -> scope mapping. `useLanguageModel(task)` predates scopes and
 * is called all over (chat panel, enhance, titles); routing the scope
 * resolution through this map wires the per-task overrides for every caller
 * centrally instead of migrating each call site to `useScopedLanguageModel`.
 * Unmapped/absent tasks resolve the plain global selection.
 */
const TASK_SCOPE: Partial<Record<CharTask, LlmScope>> = {
  chat: "chat",
  title: "notes",
  enhance: "notes",
};

export const useLanguageModel = (task?: CharTask): LanguageModelV3 | null => {
  const scope = task ? TASK_SCOPE[task] : undefined;
  const { conn } = useLLMConnection();
  const scoped = useScopedLanguageModelInner(scope ?? "cleanup", task);
  const { session } = useAuth();

  // Auth is resolved at fetch time (not model construction) so token
  // refreshes take effect without recreating the chat transport chain.
  const accessTokenRef = useRef(session?.access_token);
  accessTokenRef.current = session?.access_token;

  const globalModel = useMemo(() => {
    if (!conn) return null;

    const hostedFetch =
      conn.providerId === "hyprnote"
        ? createAuthFetch(
            task ? createTracedFetch(task) : tracedFetch,
            () => accessTokenRef.current,
          )
        : undefined;

    return createLanguageModel(conn, task, hostedFetch);
  }, [conn, task]);

  return scope ? scoped : globalModel;
};

/**
 * Scope-aware model resolution (0.5.1). Resolves the effective LLM for a
 * given scope (`chat` / `notes` / `cleanup`): its per-scope override provider
 * + model when set and honoured, otherwise the global selection. The AI-SDK
 * model is constructed by the exact same path as `useLanguageModel` (secrets
 * still flow through the provider store), so this adds no new provider code.
 *
 * The cloud-opt-in invariant is enforced in `resolveScopeSelection`: a scope
 * override can never route to a cloud provider unless cloud is already opted
 * into globally. An unknown/unavailable override, or a blocked cloud one,
 * falls back to the global selection with an explicit console warning rather
 * than silently doing nothing.
 */
export const useScopedLanguageModel = (
  scope: LlmScope,
): LanguageModelV3 | null => {
  return useScopedLanguageModelInner(scope, undefined);
};

const useScopedLanguageModelInner = (
  scope: LlmScope,
  task: CharTask | undefined,
): LanguageModelV3 | null => {
  const auth = useAuth();
  const billing = useBillingAccess();
  const session = auth?.session;

  const accessTokenRef = useRef(session?.access_token);
  accessTokenRef.current = session?.access_token;

  const keys = SCOPE_SETTING_KEYS[scope];
  const cfg = useConfigValues([
    "current_llm_provider",
    "current_llm_model",
    keys.provider,
    keys.model,
  ] as const);

  const globalProviderId = cfg.current_llm_provider;
  const globalModelId = cfg.current_llm_model;
  const overrideProviderId = (
    (cfg[keys.provider] as string | undefined) ?? ""
  ).trim();
  const overrideModelId = (
    (cfg[keys.model] as string | undefined) ?? ""
  ).trim();

  const globalProviderConfig = useAiProvider("llm", globalProviderId) as
    | AIProviderStorage
    | undefined;
  const overrideProviderConfig = useAiProvider(
    "llm",
    overrideProviderId || undefined,
  ) as AIProviderStorage | undefined;

  return useMemo(() => {
    const globalRes = resolveLLMConnection({
      providerId: globalProviderId,
      modelId: globalModelId,
      providerConfig: globalProviderConfig,
      session,
      isPaid: billing.isPaid,
    });

    const overrideRes = overrideProviderId
      ? resolveLLMConnection({
          providerId: overrideProviderId,
          // An override may pin a provider but inherit the global model.
          modelId: overrideModelId || globalModelId,
          providerConfig: overrideProviderConfig,
          session,
          isPaid: billing.isPaid,
        })
      : { conn: null, status: undefined as never };

    const globalIsCloud = globalRes.conn
      ? isCloudProvider(globalRes.conn.providerId, globalRes.conn.baseUrl)
      : false;
    const overrideIsCloud = isCloudProvider(
      overrideProviderId || undefined,
      overrideRes.conn?.baseUrl ?? overrideProviderConfig?.base_url,
    );

    const selection = resolveScopeSelection({
      hasOverride: overrideProviderId.length > 0,
      overrideKnown: PROVIDERS.some((p) => p.id === overrideProviderId),
      overrideAvailable: overrideRes.conn !== null,
      overrideIsCloud,
      globalIsCloud,
    });

    if (
      selection.source === "inherit" &&
      selection.fallbackReason &&
      selection.fallbackReason !== "no_override"
    ) {
      console.warn(
        `[ai] scope "${scope}" override (${overrideProviderId}) not used ` +
          `(${selection.fallbackReason}); inheriting the global selection`,
      );
    }

    const conn =
      selection.source === "override" ? overrideRes.conn : globalRes.conn;
    if (!conn) {
      return null;
    }

    const hostedFetch =
      conn.providerId === "hyprnote"
        ? createAuthFetch(
            task ? createTracedFetch(task) : tracedFetch,
            () => accessTokenRef.current,
          )
        : undefined;

    return createLanguageModel(conn, task, hostedFetch);
  }, [
    scope,
    task,
    globalProviderId,
    globalModelId,
    overrideProviderId,
    overrideModelId,
    globalProviderConfig,
    overrideProviderConfig,
    session,
    billing.isPaid,
  ]);
};

export const useLLMConnection = (): LLMConnectionResult => {
  const auth = useAuth();
  const billing = useBillingAccess();

  const { current_llm_provider, current_llm_model } = useConfigValues([
    "current_llm_provider",
    "current_llm_model",
  ] as const);
  const providerConfig = useAiProvider("llm", current_llm_provider) as
    | AIProviderStorage
    | undefined;

  return useMemo<LLMConnectionResult>(
    () =>
      resolveLLMConnection({
        providerId: current_llm_provider,
        modelId: current_llm_model,
        providerConfig,
        session: auth?.session,
        isPaid: billing.isPaid,
      }),
    [
      auth,
      billing.isPaid,
      current_llm_model,
      current_llm_provider,
      providerConfig,
    ],
  );
};

export const useLLMConnectionStatus = (): LLMConnectionStatus => {
  const { status } = useLLMConnection();
  return status;
};

const resolveLLMConnection = (params: {
  providerId: string | undefined;
  modelId: string | undefined;
  providerConfig: AIProviderStorage | undefined;
  session: { access_token: string } | null | undefined;
  isPaid: boolean;
}): LLMConnectionResult => {
  const {
    providerId: rawProviderId,
    modelId,
    providerConfig,
    session,
    isPaid,
  } = params;

  if (!rawProviderId) {
    return {
      conn: null,
      status: { status: "pending", reason: "missing_provider" },
    };
  }

  const providerId = rawProviderId as ProviderId;

  if (!modelId) {
    return {
      conn: null,
      status: { status: "pending", reason: "missing_model", providerId },
    };
  }

  const providerDefinition = PROVIDERS.find((p) => p.id === rawProviderId);

  if (!providerDefinition) {
    return {
      conn: null,
      status: {
        status: "error",
        reason: "provider_not_found",
        providerId: rawProviderId,
      },
    };
  }

  const baseUrl =
    providerConfig?.base_url?.trim() ||
    providerDefinition.baseUrl?.trim() ||
    "";
  const apiKey = providerConfig?.api_key?.trim() || "";

  const context: ProviderEligibilityContext = {
    isAuthenticated: !!session,
    isPaid,
    config: { base_url: baseUrl, api_key: apiKey },
  };

  const blockers = getProviderSelectionBlockers(
    providerDefinition.requirements,
    context,
  );

  if (blockers.length > 0) {
    const blocker = blockers[0];
    if (blocker.code === "requires_auth" && providerId === "hyprnote") {
      return {
        conn: null,
        status: { status: "error", reason: "unauthenticated", providerId },
      };
    }
    if (blocker.code === "requires_entitlement" && providerId === "hyprnote") {
      return {
        conn: null,
        status: { status: "error", reason: "not_pro", providerId },
      };
    }
    if (blocker.code === "missing_config") {
      return {
        conn: null,
        status: {
          status: "error",
          reason: "missing_config",
          providerId,
          missing: blocker.fields,
        },
      };
    }
  }

  if (providerId === "hyprnote" && session) {
    return {
      conn: {
        providerId,
        modelId,
        baseUrl: baseUrl ?? new URL("/llm", env.VITE_API_URL).toString(),
        apiKey: session.access_token,
      },
      status: { status: "success", providerId, isHosted: true },
    };
  }

  return {
    conn: { providerId, modelId, baseUrl, apiKey },
    status: { status: "success", providerId, isHosted: false },
  };
};

const wrapWithThinkingMiddleware = (
  model: LanguageModelV3,
): LanguageModelV3 => {
  return wrapLanguageModel({
    model,
    middleware: [
      extractReasoningMiddleware({ tagName: "think" }),
      extractReasoningMiddleware({ tagName: "thinking" }),
    ],
  });
};

const createLanguageModel = (
  conn: LLMConnectionInfo,
  task?: CharTask,
  hostedFetch?: typeof fetch,
): LanguageModelV3 => {
  switch (conn.providerId) {
    case "hyprnote": {
      const provider = createOpenRouter({
        fetch: hostedFetch ?? (task ? createTracedFetch(task) : tracedFetch),
        baseURL: conn.baseUrl,
        apiKey: conn.apiKey,
      });
      return wrapWithThinkingMiddleware(provider.chat(conn.modelId));
    }

    case "anthropic": {
      const provider = createAnthropic({
        fetch: tauriFetch,
        apiKey: conn.apiKey,
        headers: {
          "anthropic-version": "2023-06-01",
          "anthropic-dangerous-direct-browser-access": "true",
        },
      });
      return wrapWithThinkingMiddleware(provider(conn.modelId));
    }

    case "google_generative_ai": {
      const provider = createGoogleGenerativeAI({
        fetch: tauriFetch,
        baseURL: conn.baseUrl,
        apiKey: conn.apiKey,
      });
      return wrapWithThinkingMiddleware(provider(conn.modelId));
    }

    case "openrouter": {
      const provider = createOpenRouter({
        fetch: tauriFetch,
        apiKey: conn.apiKey,
      });
      return wrapWithThinkingMiddleware(provider.chat(conn.modelId));
    }

    case "openai": {
      const provider = createOpenAI({
        fetch: tauriFetch,
        baseURL: conn.baseUrl,
        apiKey: conn.apiKey,
      });
      return wrapWithThinkingMiddleware(provider(conn.modelId));
    }

    case "azure_openai": {
      const provider = createAzure({
        fetch: tauriFetch,
        baseURL: conn.baseUrl,
        apiKey: conn.apiKey,
      });
      return wrapWithThinkingMiddleware(provider(conn.modelId));
    }

    case "azure_ai": {
      const provider = createOpenAICompatible({
        fetch: tauriFetch,
        name: "azure_ai",
        baseURL: conn.baseUrl,
        apiKey: conn.apiKey,
        headers: { "api-key": conn.apiKey },
      });
      return wrapWithThinkingMiddleware(provider.chatModel(conn.modelId));
    }

    case "ollama": {
      const ollamaOrigin = new URL(conn.baseUrl.replace(/\/v1\/?$/, "")).origin;
      const ollamaFetch: typeof fetch = async (input, init) => {
        const headers = new Headers(init?.headers);
        headers.set("Origin", ollamaOrigin);
        return tauriFetch(input as RequestInfo | URL, {
          ...init,
          headers,
        });
      };
      const provider = createOpenAICompatible({
        fetch: ollamaFetch,
        name: conn.providerId,
        baseURL: conn.baseUrl,
      });
      return wrapWithThinkingMiddleware(provider.chatModel(conn.modelId));
    }

    default: {
      const config: Parameters<typeof createOpenAICompatible>[0] = {
        fetch: tauriFetch,
        name: conn.providerId,
        baseURL: conn.baseUrl,
      };
      if (conn.apiKey) {
        config.apiKey = conn.apiKey;
      }
      const provider = createOpenAICompatible(config);
      return wrapWithThinkingMiddleware(provider.chatModel(conn.modelId));
    }
  }
};

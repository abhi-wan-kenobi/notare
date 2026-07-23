/**
 * LLM routing layer (WS-A, 0.5). Frozen interface: `resolveModel(task, ctx)`.
 *
 * One place decides which model serves which task, and enforces the two
 * product invariants:
 *
 * 1. **Local-first / cloud only on explicit opt-in** (the privacy moat).
 *    A cloud-tier provider (BYO cloud or hosted) is only ever returned when
 *    the user has explicitly selected that provider in settings
 *    (`ctx.selectionIsExplicit`). The router NEVER falls back to a cloud
 *    tier on its own: fallback candidates are filtered to `local` tier
 *    unconditionally.
 * 2. **Capability gating** — tasks with hard requirements (action_items:
 *    structured outputs + ≥7B) refuse under-capable models up-front instead
 *    of emitting garbage that downstream verbatim-gates reject wholesale.
 *
 * The core is a pure function over an explicit context so the invariants are
 * unit-testable; `useTaskModel` is the React binding that assembles the
 * context from the existing hooks (auth/billing/config/live-queries) and
 * constructs the AI-SDK `LanguageModel` exactly as `useLanguageModel` did —
 * the model-construction path is unchanged (secrets stay on store2/keyring).
 */

import { useMemo } from "react";

import type { CharTask } from "@hypr/api-client";

import { type CapsCheck, checkCaps, type LlmTask } from "./capabilities";

import { useLanguageModel, useLLMConnection } from "~/ai/hooks";
import { type ProviderId, PROVIDERS } from "~/settings/ai/llm/shared";
import { useConfigValues } from "~/shared/config";

export type { CapsCheck, LlmTask };
export {
  checkCaps,
  inferParamsB,
  providerSupportsStructuredOutputs,
  TASK_REQUIREMENTS,
} from "./capabilities";

export type ModelTier = "local" | "byo-cloud" | "hosted";

/** A provider+model pair the router can consider. */
export type Candidate = {
  providerId: ProviderId | string;
  modelId: string;
  /** Endpoint base URL — used for tier classification of `custom`. */
  baseUrl?: string;
};

export type ResolveContext = {
  /** The user's currently-selected connection (settings), if any. */
  selected: Candidate | null;
  /**
   * Whether `selected` came from an explicit user choice in settings.
   * Today this is always true when `selected` is non-null (the config values
   * are only ever written by the settings UI); it is a named flag so future
   * auto-discovery paths cannot silently count as consent.
   */
  selectionIsExplicit: boolean;
  /**
   * Additional candidates from local discovery (ollama/LM Studio probes).
   * The router hard-filters these to `local` tier — a cloud entry here is a
   * caller bug and is dropped, never returned.
   */
  localFallbacks?: Candidate[];
  /** User setting: bypass the minimum-size heuristic for structured tasks. */
  capsUserOverride?: boolean;
};

export type Resolution = {
  status: "ok";
  providerId: ProviderId | string;
  modelId: string;
  tier: ModelTier;
  caps: CapsCheck;
  /** True when caps passed only via "unknown" (surface a warning in UI). */
  capsUncertain: boolean;
};

export type ResolutionFailure = {
  status: "unavailable";
  reason:
    | "no_provider" // nothing selected, no local fallback
    | "cloud_not_opted_in" // selected is cloud-tier without explicit selection
    | "caps_unmet"; // task requirements provably not met
  /** For caps_unmet: what failed, so the UI can explain. */
  caps?: CapsCheck;
};

export type ResolveResult = Resolution | ResolutionFailure;

const LOCAL_PROVIDER_IDS: ReadonlySet<string> = new Set(["ollama", "lmstudio"]);
const HOSTED_PROVIDER_IDS: ReadonlySet<string> = new Set(["hyprnote"]);

/** RFC1918 / loopback / mDNS-local hosts count as local endpoints. */
export function isLocalUrl(baseUrl: string | undefined): boolean {
  if (!baseUrl) return false;
  try {
    const host = new URL(baseUrl).hostname;
    if (
      host === "localhost" ||
      host === "127.0.0.1" ||
      host === "::1" ||
      host === "[::1]"
    ) {
      return true;
    }
    if (
      host.endsWith(".local") ||
      host.endsWith(".internal") ||
      host.endsWith(".ts.net")
    ) {
      return true;
    }
    if (/^10\.\d+\.\d+\.\d+$/.test(host)) return true;
    if (/^192\.168\.\d+\.\d+$/.test(host)) return true;
    const m = /^172\.(\d+)\.\d+\.\d+$/.exec(host);
    if (m) {
      const second = Number(m[1]);
      return second >= 16 && second <= 31;
    }
    return false;
  } catch {
    return false;
  }
}

export function classifyTier(candidate: Candidate): ModelTier {
  const id = candidate.providerId;
  if (LOCAL_PROVIDER_IDS.has(id)) return "local";
  if (HOSTED_PROVIDER_IDS.has(id)) return "hosted";
  if (id === "custom") {
    return isLocalUrl(candidate.baseUrl) ? "local" : "byo-cloud";
  }
  return "byo-cloud";
}

/**
 * Pure resolution core. See module docs for the invariants.
 */
export function resolveModel(
  task: LlmTask,
  ctx: ResolveContext,
): ResolveResult {
  const candidates: Array<{ candidate: Candidate; explicit: boolean }> = [];

  if (ctx.selected) {
    candidates.push({
      candidate: ctx.selected,
      explicit: ctx.selectionIsExplicit,
    });
  }
  for (const fb of ctx.localFallbacks ?? []) {
    // Invariant 1 enforcement: non-local fallbacks are dropped, never used.
    if (classifyTier(fb) === "local") {
      candidates.push({ candidate: fb, explicit: false });
    }
  }

  if (candidates.length === 0) {
    return { status: "unavailable", reason: "no_provider" };
  }

  let lastCapsFailure: CapsCheck | undefined;
  let sawCloudWithoutOptIn = false;

  for (const { candidate, explicit } of candidates) {
    const tier = classifyTier(candidate);

    if (tier !== "local" && !explicit) {
      sawCloudWithoutOptIn = true;
      continue;
    }

    const caps = checkCaps({
      task,
      providerId: candidate.providerId,
      modelId: candidate.modelId,
      userOverride: ctx.capsUserOverride,
    });

    const structuredOk = caps.structuredOutputs !== false;
    const sizeOk = caps.minParamsOk !== false;
    if (!structuredOk || !sizeOk) {
      lastCapsFailure = caps;
      continue;
    }

    return {
      status: "ok",
      providerId: candidate.providerId,
      modelId: candidate.modelId,
      tier,
      caps,
      capsUncertain:
        caps.structuredOutputs === "unknown" || caps.minParamsOk === "unknown",
    };
  }

  if (lastCapsFailure) {
    return {
      status: "unavailable",
      reason: "caps_unmet",
      caps: lastCapsFailure,
    };
  }
  if (sawCloudWithoutOptIn) {
    return { status: "unavailable", reason: "cloud_not_opted_in" };
  }
  return { status: "unavailable", reason: "no_provider" };
}

/** Tasks that exist in the generated tracing union get the tracing header. */
function asCharTask(task: LlmTask): CharTask | undefined {
  return task === "chat" || task === "enhance" || task === "title"
    ? task
    : undefined;
}

/**
 * Provider routing target for the structured-output path. When present and
 * `providerId === "ollama"`, `generateStructured`/`extractActionItems` use
 * ollama's native `/api/chat` `format` endpoint (reasoning models return prose
 * to the openai-compat path). `null` when nothing is resolved.
 */
export type ModelTarget = {
  providerId: string;
  modelId: string;
  baseUrl: string;
};

export type TaskModel = {
  model: ReturnType<typeof useLanguageModel>;
  resolution: ResolveResult;
  /** Non-null only when resolution succeeded; feeds `deps.target`. */
  target: ModelTarget | null;
};

/**
 * React binding: resolve `task` against the user's current selection and
 * construct the AI-SDK model (same construction path as before — this hook
 * wraps `useLanguageModel`, it does not reimplement providers or touch
 * secrets). Returns `model: null` whenever resolution fails, with the
 * structured `resolution` explaining why.
 */
export function useTaskModel(task: LlmTask): TaskModel {
  const { conn } = useLLMConnection();
  const { llm_caps_override } = useConfigValues(["llm_caps_override"] as const);
  const model = useLanguageModel(asCharTask(task));

  return useMemo(() => {
    const providerDef = conn
      ? PROVIDERS.find((p) => p.id === conn.providerId)
      : undefined;
    const resolution = resolveModel(task, {
      selected: conn
        ? {
            providerId: conn.providerId,
            modelId: conn.modelId,
            baseUrl: conn.baseUrl ?? providerDef?.baseUrl,
          }
        : null,
      // conn is only ever populated from settings the user wrote (see
      // ResolveContext docs); discovery-sourced candidates must NOT set this.
      selectionIsExplicit: conn !== null,
      capsUserOverride: llm_caps_override === true,
    });

    const target: ModelTarget | null =
      resolution.status === "ok" && conn
        ? {
            providerId: conn.providerId,
            modelId: conn.modelId,
            baseUrl: conn.baseUrl ?? providerDef?.baseUrl ?? "",
          }
        : null;

    return {
      model: resolution.status === "ok" ? model : null,
      resolution,
      target,
    };
  }, [conn, task, model, llm_caps_override]);
}

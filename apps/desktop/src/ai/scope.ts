/**
 * Per-scope LLM configuration (0.5.1). Each "scope" - the chat panel, notes
 * enhancement, dictation cleanup - may pin its own provider + model, or leave
 * the override empty to inherit the global selection
 * (`current_llm_provider` / `current_llm_model`).
 *
 * This module holds the PURE resolution decision (which is unit-testable in
 * isolation); the React binding + AI-SDK model construction live in
 * `~/ai/hooks` (`useScopedLanguageModel`), reusing the exact same connection
 * + provider code path as the global `useLanguageModel`.
 *
 * The one invariant that must never regress: **a per-scope override may only
 * route to a cloud provider when cloud is already opted into globally.** A
 * scope override can never become a back-door that ships dictation (or notes,
 * or chat) to a cloud endpoint the user never explicitly chose. When the gate
 * (or availability) fails, we fall back to the global selection - explicitly,
 * with a log - rather than silently inheriting a capability gap.
 */

export const LLM_SCOPES = ["chat", "notes", "cleanup"] as const;
export type LlmScope = (typeof LLM_SCOPES)[number];

/** The two schema keys backing each scope's override. */
export const SCOPE_SETTING_KEYS = {
  chat: { provider: "ai_scope_chat_provider", model: "ai_scope_chat_model" },
  notes: { provider: "ai_scope_notes_provider", model: "ai_scope_notes_model" },
  cleanup: {
    provider: "ai_scope_cleanup_provider",
    model: "ai_scope_cleanup_model",
  },
} as const satisfies Record<LlmScope, { provider: string; model: string }>;

/** Local inference engines + loopback/LAN custom endpoints are NOT cloud. */
const LOCAL_PROVIDER_IDS: ReadonlySet<string> = new Set(["ollama", "lmstudio"]);

/**
 * Mirror of the llm-router's `isLocalUrl` (kept standalone here to avoid an
 * import cycle: llm-router already imports `~/ai/hooks`). RFC1918 / loopback /
 * mDNS-and-tailnet-local hosts count as local endpoints.
 */
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

/**
 * Whether a provider+endpoint is a cloud tier (anything not provably local).
 * `custom` is local only when its base URL is a local host; every other
 * non-local provider (incl. the hosted `hyprnote` tier) is cloud.
 */
export function isCloudProvider(
  providerId: string | undefined,
  baseUrl: string | undefined,
): boolean {
  if (!providerId) return false;
  if (LOCAL_PROVIDER_IDS.has(providerId)) return false;
  if (providerId === "custom") return !isLocalUrl(baseUrl);
  return true;
}

export type ScopeFallbackReason =
  | "no_override" // no override configured - normal inheritance
  | "unknown_provider" // override names a provider that doesn't exist
  | "unavailable" // override provider/model can't form a connection
  | "cloud_not_opted_in"; // override is cloud but cloud isn't opted in globally

export interface ScopeSelectionInput {
  /** An override provider id is set (non-empty). */
  hasOverride: boolean;
  /** The override provider id is a known/registered provider. */
  overrideKnown: boolean;
  /** The override forms a usable connection (config present, model set). */
  overrideAvailable: boolean;
  /** The override resolves to a cloud-tier endpoint. */
  overrideIsCloud: boolean;
  /** The global selection is itself a cloud provider (= cloud opted in). */
  globalIsCloud: boolean;
}

export interface ScopeSelection {
  source: "override" | "inherit";
  /** Populated whenever an override was requested but not honoured. */
  fallbackReason?: ScopeFallbackReason;
}

/**
 * Decide whether a scope uses its override or inherits the global selection.
 * Pure and total; see the module invariant. Order matters: the cloud-opt-in
 * gate is checked BEFORE availability so a cloud override without global
 * opt-in is always reported as `cloud_not_opted_in` (the security-relevant
 * reason), never masked by an incidental availability miss.
 */
export function resolveScopeSelection(
  input: ScopeSelectionInput,
): ScopeSelection {
  if (!input.hasOverride) {
    return { source: "inherit", fallbackReason: "no_override" };
  }
  if (!input.overrideKnown) {
    return { source: "inherit", fallbackReason: "unknown_provider" };
  }
  // INVARIANT: an override may only reach a cloud provider when cloud is
  // already opted into globally.
  if (input.overrideIsCloud && !input.globalIsCloud) {
    return { source: "inherit", fallbackReason: "cloud_not_opted_in" };
  }
  if (!input.overrideAvailable) {
    return { source: "inherit", fallbackReason: "unavailable" };
  }
  return { source: "override" };
}

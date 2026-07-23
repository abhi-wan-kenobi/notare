import type { ProviderId } from "~/settings/ai/llm/shared";

/**
 * Tasks the router can resolve a model for (WS-A frozen interface).
 * Supersets the generated `CharTask` ('chat' | 'enhance' | 'title') with the
 * 0.5 structured-output tasks.
 */
export type LlmTask =
  | "chat"
  | "enhance"
  | "title"
  | "action_items"
  | "dictation_cleanup";

/** Per-task model requirements. */
export type TaskRequirements = {
  /** Task needs provider-enforced structured outputs (JSON-schema decoding). */
  structuredOutputs: boolean;
  /**
   * Minimum parameter count (billions) below which output quality is known to
   * be unacceptable for the task. `null` = no floor.
   */
  minParamsB: number | null;
};

/**
 * Requirements table (0.5 scope): action items get the 7B floor +
 * structured-output requirement from the roadmap research — smaller models
 * hallucinate owners/dates at rates the verbatim-source gate then rejects
 * wholesale, so refusing up-front gives a better UX than emitting garbage.
 */
export const TASK_REQUIREMENTS: Record<LlmTask, TaskRequirements> = {
  chat: { structuredOutputs: false, minParamsB: null },
  enhance: { structuredOutputs: false, minParamsB: null },
  title: { structuredOutputs: false, minParamsB: null },
  action_items: { structuredOutputs: true, minParamsB: 7 },
  dictation_cleanup: { structuredOutputs: false, minParamsB: null },
};

/**
 * Providers whose API path supports schema-constrained decoding
 * (`generateObject` with a JSON schema) as used by the AI SDK:
 * - ollama: `format: <json schema>` on /api and OpenAI-compat response_format
 * - lmstudio: OpenAI-compat `response_format: { type: "json_schema", … }`
 * - the listed clouds: native structured outputs
 * - custom: unknown endpoint — treated as capable only via user override
 */
const STRUCTURED_OUTPUT_PROVIDERS: ReadonlySet<string> = new Set([
  "ollama",
  "lmstudio",
  "openai",
  "azure_openai",
  "azure_ai",
  "anthropic",
  "google_generative_ai",
  "openrouter",
  "mistral",
  "hyprnote",
]);

export function providerSupportsStructuredOutputs(
  providerId: ProviderId | string,
): boolean | "unknown" {
  if (STRUCTURED_OUTPUT_PROVIDERS.has(providerId)) {
    return true;
  }
  if (providerId === "custom" || providerId === "cloudflare_workers_ai") {
    return "unknown";
  }
  return "unknown";
}

/**
 * Best-effort parameter-count (billions) heuristic from a model id.
 * Understands the ollama/LM Studio naming conventions:
 *   "qwen3:8b", "llama3.1:70b-instruct-q4_K_M", "gemma3:27b-it",
 *   "Meta-Llama-3.1-8B-Instruct-GGUF", "phi-4-14b", "mistral-7b-v0.3"
 * Returns `null` when no size can be inferred ("unknown", NOT zero) — e.g.
 * hosted ids like "gpt-4o" or "claude-sonnet".
 *
 * Guards against false positives: quantization suffixes (q4, q8_0, 4bit),
 * context markers (128k), and version fragments ("v0.3") are not sizes.
 */
export function inferParamsB(modelId: string): number | null {
  const id = modelId.toLowerCase();
  // "<number>b" with a non-alphanumeric (and non-dot, so "3.1" version
  // fragments can't bleed in) boundary before, and no alphanumeric after —
  // matches "qwen3:8b", "…-0.6b", "…-70b-instruct-q4_K_M", "…-8b-…";
  // does not match "q8_0" (no b), "128k" (no b), or "70bit" (trailing alnum).
  const re = /(?:^|[^0-9a-z.])(\d+(?:\.\d+)?)b(?![a-z0-9])/g;
  for (const m of id.matchAll(re)) {
    const n = Number(m[1]);
    // Plausibility window: 0.1B .. 2000B. Anything else is a context length
    // or a version fragment, not a parameter count.
    if (n >= 0.1 && n <= 2000) {
      return n;
    }
  }
  return null;
}

export type CapsCheck = {
  structuredOutputs: boolean | "unknown";
  /**
   * true  — meets the floor
   * false — provably below the floor
   * "unknown" — size not inferable from the id; allowed only with the user
   *             override (settings) and surfaced as a warning by the UI.
   */
  minParamsOk: boolean | "unknown";
};

export function checkCaps(params: {
  task: LlmTask;
  providerId: ProviderId | string;
  modelId: string;
  /** User setting: "my model is capable, stop second-guessing me". */
  userOverride?: boolean;
}): CapsCheck {
  const req = TASK_REQUIREMENTS[params.task];

  const structured = req.structuredOutputs
    ? providerSupportsStructuredOutputs(params.providerId)
    : true;

  let minParamsOk: boolean | "unknown" = true;
  if (req.minParamsB !== null) {
    const size = inferParamsB(params.modelId);
    if (size === null) {
      minParamsOk = params.userOverride ? true : "unknown";
    } else {
      minParamsOk = size >= req.minParamsB || params.userOverride === true;
    }
  }

  return { structuredOutputs: structured, minParamsOk };
}

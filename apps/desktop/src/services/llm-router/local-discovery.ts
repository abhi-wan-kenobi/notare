/**
 * Local-endpoint discovery for the llm-router (WS-A).
 *
 * The ONE place that probes local LLM endpoints (ollama, LM Studio). Both
 * engines expose OpenAI-compatible APIs, so listing is a GET `/v1/models` and
 * the structured-output capability probe is a POST `/v1/chat/completions` with a
 * `response_format` json_schema. Every probe is best-effort: short timeout,
 * return empty/false on any error — discovery is a convenience, never a hard
 * dependency, and never a consent signal.
 *
 * Discovery results feed `ResolveContext.localFallbacks` ONLY. The router
 * treats every discovered candidate as non-explicit (see the
 * `selectionIsExplicit` docs in ./index.ts), so a discovered endpoint can
 * never unlock a cloud tier.
 */

import { fetch as tauriFetch } from "@tauri-apps/plugin-http";

import type { Candidate } from "./index";

import { listOllamaModels } from "~/settings/ai/shared/list-ollama";

// Re-exported so router consumers have a single import point; the shared
// module remains the implementation (other settings callers use it directly).
export { listOllamaModels } from "~/settings/ai/shared/list-ollama";

export const OLLAMA_DEFAULT_BASE_URL = "http://localhost:11434";
export const LMSTUDIO_DEFAULT_BASE_URL = "http://localhost:1234/v1";

/** Connect timeout for local probes — these are localhost, so keep it short. */
const PROBE_CONNECT_TIMEOUT_MS = 2500;

/**
 * Fetch signature that accepts the Tauri HTTP plugin's `ClientOptions` (notably
 * `connectTimeout`) on top of the standard `RequestInit`. Injected for tests;
 * the default is the Tauri plugin fetch.
 */
type ProbeInit = RequestInit & { connectTimeout?: number };
type ProbeFetch = (
  input: string | URL | Request,
  init?: ProbeInit,
) => Promise<Response>;

/**
 * Resolve the OpenAI-compatible chat-completions URL for a base URL that may
 * or may not already end in `/v1` (ollama: `http://localhost:11434`,
 * LM Studio: `http://localhost:1234/v1`).
 */
function chatCompletionsUrl(baseUrl: string): string {
  const trimmed = baseUrl.replace(/\/+$/, "");
  if (/\/v1$/.test(trimmed)) {
    return `${trimmed}/chat/completions`;
  }
  return `${trimmed}/v1/chat/completions`;
}

/**
 * List models from an LM Studio server via the OpenAI-compatible
 * `/v1/models` endpoint. Returns `[]` on any error — never throws.
 */
export async function listLmStudioModels(
  baseUrl: string,
  fetchImpl: ProbeFetch = tauriFetch,
): Promise<string[]> {
  if (!baseUrl) return [];
  const url = `${baseUrl.replace(/\/+$/, "")}/models`;
  try {
    const res = await fetchImpl(url, {
      method: "GET",
      connectTimeout: PROBE_CONNECT_TIMEOUT_MS,
    });
    if (!res.ok) return [];
    const json = (await res.json()) as { data?: Array<{ id?: unknown }> };
    if (!json || !Array.isArray(json.data)) return [];
    return json.data
      .map((entry) => entry?.id)
      .filter((id): id is string => typeof id === "string");
  } catch {
    return [];
  }
}

/**
 * Probe ollama + LM Studio on their default localhost ports and return the
 * union as router `Candidate[]` (providerId "ollama" / "lmstudio"). A failed
 * or absent endpoint contributes no candidates — never throws.
 */
export async function discoverLocalModels(
  fetchImpl: ProbeFetch = tauriFetch,
): Promise<Candidate[]> {
  const [ollama, lmstudio] = await Promise.all([
    (async () => {
      try {
        const result = await listOllamaModels(OLLAMA_DEFAULT_BASE_URL, "");
        return result.models.map((modelId) => ({
          providerId: "ollama" as const,
          modelId,
          baseUrl: OLLAMA_DEFAULT_BASE_URL,
        }));
      } catch {
        return [];
      }
    })(),
    (async () => {
      const models = await listLmStudioModels(
        LMSTUDIO_DEFAULT_BASE_URL,
        fetchImpl,
      );
      return models.map((modelId) => ({
        providerId: "lmstudio" as const,
        modelId,
        baseUrl: LMSTUDIO_DEFAULT_BASE_URL,
      }));
    })(),
  ]);

  return [...ollama, ...lmstudio];
}

/**
 * Runtime confirmation of the static `providerSupportsStructuredOutputs`
 * heuristic: issue a tiny OpenAI-compatible `/v1/chat/completions` request
 * asking for a trivial JSON object via `response_format: json_schema`, and
 * return true iff the model echoes parseable JSON matching the requested
 * shape. Short timeout; returns false on any error (HTTP, network, malformed
 * body, non-JSON content). Never throws.
 *
 * `fetchImpl` is injectable for tests (the default is the Tauri HTTP plugin
 * fetch, the same one the listing probes use).
 */
export async function probeStructuredOutputs(
  baseUrl: string,
  modelId: string,
  fetchImpl: ProbeFetch = tauriFetch,
): Promise<boolean> {
  if (!baseUrl || !modelId) return false;

  const schema = {
    type: "object" as const,
    properties: { ok: { type: "boolean" as const } },
    required: ["ok"],
    additionalProperties: false,
  };

  const body = {
    model: modelId,
    messages: [
      {
        role: "user",
        content: 'Reply with the JSON object {"ok": true} and nothing else.',
      },
    ],
    temperature: 0,
    max_tokens: 16,
    response_format: {
      type: "json_schema",
      json_schema: { name: "probe", schema, strict: true },
    },
  };

  try {
    const res = await fetchImpl(chatCompletionsUrl(baseUrl), {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(body),
      connectTimeout: PROBE_CONNECT_TIMEOUT_MS,
    });
    if (!res.ok) return false;

    const json = (await res.json()) as {
      choices?: Array<{ message?: { content?: unknown } }>;
    };
    const content = json?.choices?.[0]?.message?.content;
    if (typeof content !== "string") return false;

    const parsed = JSON.parse(content.trim()) as unknown;
    return (
      typeof parsed === "object" &&
      parsed !== null &&
      "ok" in parsed &&
      typeof (parsed as { ok: unknown }).ok === "boolean"
    );
  } catch {
    return false;
  }
}

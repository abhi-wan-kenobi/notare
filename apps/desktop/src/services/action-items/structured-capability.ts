/**
 * Structured-output capability gate for action-item extraction (WS-C, PG gate).
 *
 * `resolveModel('action_items')` already refuses providers whose *static*
 * heuristic says they can't do structured outputs (capabilities.ts). This is
 * the RUNTIME confirmation before we actually extract: it runs #89's live
 * `probeStructuredOutputs` against the endpoint so we never feed a transcript
 * to a model that will return prose.
 *
 * ollama is exempt: we drive it through its native `format` endpoint
 * (structured-generate.ts), which grammar-constrains the decode, so a probe of
 * ollama's openai-compat surface (which reasoning models fail) would be a false
 * negative. For every other provider the probe is authoritative.
 *
 * notare-local (the embedded llama.cpp server) is deliberately NOT on that
 * exemption list, even though it also grammar-constrains
 * `response_format: json_schema` (via llama.cpp's `llguidance` sampler —
 * see `local-llm-core`'s server). Unlike ollama, its `/v1/chat/completions`
 * *is* the endpoint real extraction uses — there's no separate native path
 * for the probe to miss — so the probe is expected to pass honestly rather
 * than needing ollama's convenience carve-out.
 */

import { probeStructuredOutputs } from "~/services/llm-router/local-discovery";

export type StructuredCapability =
  | { ok: true }
  | { ok: false; reason: "probe_failed" };

export async function checkStructuredCapability(
  target: { providerId: string; modelId: string; baseUrl: string },
  probe: (
    baseUrl: string,
    modelId: string,
  ) => Promise<boolean> = probeStructuredOutputs,
): Promise<StructuredCapability> {
  // Native `format` path guarantees valid JSON regardless of the openai-compat
  // probe result — don't let a false-negative probe block ollama.
  if (target.providerId === "ollama") {
    return { ok: true };
  }
  const passed = await probe(target.baseUrl, target.modelId);
  return passed ? { ok: true } : { ok: false, reason: "probe_failed" };
}

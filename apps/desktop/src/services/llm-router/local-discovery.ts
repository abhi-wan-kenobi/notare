/**
 * Local-endpoint discovery for the llm-router (WS-A).
 *
 * PR13 scope: a single import point so router consumers stop reaching into
 * `settings/ai/shared/list-ollama` and `ai/hooks/useLLMConnection` directly.
 * The full consolidation (LAN-host probing, LM Studio :1234 via
 * `local-llm-core`, discovery-fed `ResolveContext.localFallbacks`) is PR14 —
 * at which point the re-exported modules become thin wrappers around this
 * file instead of the other way around.
 *
 * Discovery results feed `ResolveContext.localFallbacks` ONLY. They are never
 * a consent signal: the router treats every discovered candidate as
 * non-explicit, so a discovered endpoint can never unlock a cloud tier
 * (see the `selectionIsExplicit` docs in ./index.ts).
 */

export { listOllamaModels } from "~/settings/ai/shared/list-ollama";

export const OLLAMA_DEFAULT_BASE_URL = "http://localhost:11434";
export const LMSTUDIO_DEFAULT_BASE_URL = "http://localhost:1234/v1";

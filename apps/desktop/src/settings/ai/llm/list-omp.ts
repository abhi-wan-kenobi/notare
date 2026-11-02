import {
  OMP_DEFAULT_BASE_URL,
  probeOmpGateway,
} from "~/services/llm-router/local-discovery";
import type { ListModelsResult } from "~/settings/ai/shared/list-common";

/**
 * Parses model entries from an OpenAI-compatible /v1/models response
 * data array (which for omp contains aliases like @smol, @slow, @plan,
 * and upstream provider/model ids).
 */
export function parseOmpModels(
  models: ReadonlyArray<string | { id?: unknown }>,
): ListModelsResult {
  const parsedIds = models
    .map((item) => (typeof item === "string" ? item : item?.id))
    .filter(
      (id): id is string => typeof id === "string" && id.trim().length > 0,
    );

  return {
    models: parsedIds,
    ignored: [],
    metadata: Object.fromEntries(
      parsedIds.map((id) => [id, { input_modalities: ["text"] }]),
    ),
  };
}

export async function fetchOmpModels(
  baseUrl: string = OMP_DEFAULT_BASE_URL,
  apiKey?: string,
  fetchImpl?: typeof fetch,
): Promise<ListModelsResult> {
  if (!baseUrl) {
    return { models: [], ignored: [], metadata: {} };
  }

  const result = await probeOmpGateway(
    baseUrl,
    apiKey,
    fetchImpl as Parameters<typeof probeOmpGateway>[2],
  );

  return parseOmpModels(result.models);
}

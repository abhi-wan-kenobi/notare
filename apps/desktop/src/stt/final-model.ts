import {
  commands as localSttCommands,
  type LocalModel,
} from "@hypr/plugin-local-stt";

import { isSupportedLocalSttModel } from "~/stt/capabilities";

// The final ("post-meeting") transcription model. Batch transcription runs —
// post-capture, file upload, and manual re-transcription — use this model
// when it is configured; otherwise they use the live model, which is the
// pre-existing behavior.

export type FinalBatchTarget = {
  model: string;
  /**
   * Base URL for the batch request. For local models this is the URL of the
   * local STT server started for the final model; for external/custom
   * providers it is the provider's configured base URL.
   */
  baseUrl?: string;
  /**
   * API key for the batch request. Only set for external/custom providers;
   * local models use the loopback server (no key).
   */
  apiKey?: string;
  /**
   * Restores the live transcription server. Only meaningful for local
   * models (a single local STT server runs at a time, so preparing the final
   * model swaps the live one out). Always safe to call.
   */
  restore: () => Promise<void>;
};

const noopRestore = async () => {};

/**
 * Checks whether a model id is a local ("hyprnote") model. Uses the fast
 * prefix check first and falls back to the local-stt plugin's supported-model
 * catalog, so new model families (e.g. future engines with unknown prefixes)
 * work without hardcoding names here.
 */
async function isKnownLocalSttModel(model: string): Promise<boolean> {
  if (isSupportedLocalSttModel(model)) {
    return true;
  }

  try {
    const supported = await localSttCommands.listSupportedModels();
    return (
      supported.status === "ok" &&
      supported.data.some((info) => info.key === model)
    );
  } catch {
    return false;
  }
}

export type FinalSttModelOption = {
  key: string;
  displayName: string;
};

/**
 * Lists the downloaded local models that are recommended for final-pass
 * (batch) transcription — `recommended_use` of "final" or "liveAndFinal".
 * Drives the "Re-transcribe with…" picker. Never throws; returns an empty
 * list when the local-stt plugin is unavailable.
 */
export async function listDownloadedFinalSttModels(): Promise<
  FinalSttModelOption[]
> {
  try {
    const supported = await localSttCommands.listSupportedModels();
    if (supported.status !== "ok") {
      return [];
    }

    const finalModels = supported.data.filter(
      (info) =>
        info.recommended_use === "final" ||
        info.recommended_use === "liveAndFinal",
    );

    const downloaded = await Promise.all(
      finalModels.map(async (info) => {
        try {
          const result = await localSttCommands.isModelDownloaded(info.key);
          return result.status === "ok" && result.data ? info : null;
        } catch {
          return null;
        }
      }),
    );

    return downloaded
      .filter((info) => info !== null)
      .map((info) => ({ key: info.key, displayName: info.display_name }));
  } catch (error) {
    console.warn("[stt] failed to list downloaded final models", error);
    return [];
  }
}

/**
 * Decides whether a distinct final-pass model should be used.
 * Returns the final model id, or null to keep using the live model.
 */
export function pickFinalSttModel({
  finalSetting,
  liveModel,
}: {
  finalSetting: unknown;
  liveModel: string | null | undefined;
}): string | null {
  if (typeof finalSetting !== "string") {
    return null;
  }

  const finalModel = finalSetting.trim();
  if (!finalModel || finalModel === liveModel) {
    return null;
  }

  return finalModel;
}

/**
 * Prepares the batch-transcription target for the final model.
 *
 * - External/custom providers: the caller supplies the provider's configured
 *   base URL / API key (independent of the live connection, so batch can run
 *   on a remote/custom server while live stays local); a different model id
 *   is used.
 * - Local ("hyprnote") models: verifies the model is downloaded and starts
 *   the local STT server for it (this stops the live model's server; the
 *   returned `restore` brings it back).
 *
 * Returns null when the final model cannot be used — callers fall back to
 * the live model, i.e. today's behavior.
 */
export async function resolveFinalBatchTarget({
  provider,
  liveModel,
  finalModel,
  finalBaseUrl,
  finalApiKey,
}: {
  provider: string;
  liveModel: string | null | undefined;
  finalModel: string;
  finalBaseUrl?: string;
  finalApiKey?: string;
}): Promise<FinalBatchTarget | null> {
  if (provider !== "hyprnote") {
    return {
      model: finalModel,
      baseUrl: finalBaseUrl,
      apiKey: finalApiKey,
      restore: noopRestore,
    };
  }

  if (finalModel === "cloud" || !(await isKnownLocalSttModel(finalModel))) {
    return null;
  }

  try {
    const downloaded = await localSttCommands.isModelDownloaded(
      finalModel as LocalModel,
    );
    if (downloaded.status !== "ok" || !downloaded.data) {
      console.warn(
        `[stt] final model "${finalModel}" is not downloaded; using the live model instead`,
      );
      return null;
    }

    const server = await localSttCommands.startServer(finalModel as LocalModel);
    if (server.status !== "ok") {
      console.warn(
        `[stt] failed to start server for final model "${finalModel}"; using the live model instead`,
        server.error,
      );
      return null;
    }

    // Decide up front which server `restore` should bring back: the live
    // local model when there is one, otherwise just stop the final server.
    const restoreModel =
      typeof liveModel === "string" &&
      liveModel !== "cloud" &&
      (await isKnownLocalSttModel(liveModel))
        ? (liveModel as LocalModel)
        : null;

    return {
      model: finalModel,
      baseUrl: server.data,
      restore: async () => {
        try {
          if (restoreModel) {
            await localSttCommands.startServer(restoreModel);
          } else {
            await localSttCommands.stopServer(null);
          }
        } catch (error) {
          console.error(
            "[stt] failed to restore live transcription server",
            error,
          );
        }
      },
    };
  } catch (error) {
    console.warn(
      "[stt] failed to prepare final transcription model; using the live model instead",
      error,
    );
    return null;
  }
}

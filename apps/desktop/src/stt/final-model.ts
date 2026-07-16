import { commands as localSttCommands } from "@hypr/plugin-local-stt";

import {
  isHyprnoteLocalSttModel,
  isSupportedLocalSttModel,
} from "~/stt/capabilities";

// The final ("post-meeting") transcription model. Batch transcription runs —
// post-capture, file upload, and manual re-transcription — use this model
// when it is configured; otherwise they use the live model, which is the
// pre-existing behavior.

export type FinalBatchTarget = {
  model: string;
  /**
   * Only set for local models: the URL of the local STT server started for
   * the final model. External providers reuse the live connection's base URL.
   */
  baseUrl?: string;
  /**
   * Restores the live transcription server. Only meaningful for local
   * models (a single local STT server runs at a time, so preparing the final
   * model swaps the live one out). Always safe to call.
   */
  restore: () => Promise<void>;
};

const noopRestore = async () => {};

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
 * - External providers: same base URL / API key, different model id.
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
}: {
  provider: string;
  liveModel: string | null | undefined;
  finalModel: string;
}): Promise<FinalBatchTarget | null> {
  if (provider !== "hyprnote") {
    return { model: finalModel, restore: noopRestore };
  }

  if (!isSupportedLocalSttModel(finalModel)) {
    return null;
  }

  try {
    const downloaded = await localSttCommands.isModelDownloaded(finalModel);
    if (downloaded.status !== "ok" || !downloaded.data) {
      console.warn(
        `[stt] final model "${finalModel}" is not downloaded; using the live model instead`,
      );
      return null;
    }

    const server = await localSttCommands.startServer(finalModel);
    if (server.status !== "ok") {
      console.warn(
        `[stt] failed to start server for final model "${finalModel}"; using the live model instead`,
        server.error,
      );
      return null;
    }

    return {
      model: finalModel,
      baseUrl: server.data,
      restore: async () => {
        try {
          if (isHyprnoteLocalSttModel(provider, liveModel)) {
            await localSttCommands.startServer(liveModel);
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

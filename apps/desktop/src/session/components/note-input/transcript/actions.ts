import { ask } from "@tauri-apps/plugin-dialog";
import { useCallback } from "react";

import { commands as fsSyncCommands } from "@hypr/plugin-fs-sync";

import { getEnhancerService } from "~/services/enhancer";
import { showTransientToast } from "~/sidebar/toast/transient";
import { useListener } from "~/stt/contexts";
import { resolveFinalBatchTarget } from "~/stt/final-model";
import { isStoppedTranscriptionError, useRunBatch } from "~/stt/useRunBatch";
import { useSTTConnection } from "~/stt/useSTTConnection";

export function confirmRetranscribe() {
  return ask(
    "Re-transcribe this recording? The current transcript will be replaced.",
    {
      title: "Re-transcribe recording",
      kind: "warning",
      okLabel: "Re-transcribe",
      cancelLabel: "Cancel",
    },
  );
}

// Re-runs transcription of the stored session audio and replaces the session
// transcript. The batch path uses the configured final model (falling back to
// the live model), and the transcript replacement lands in transcript.md on
// disk through the fs-materializer's existing DB-to-disk sync.
export function useRegenerateTranscript(sessionId: string) {
  const runBatch = useRunBatch(sessionId);
  const handleBatchFailed = useListener((state) => state.handleBatchFailed);

  return useCallback(async () => {
    const confirmed = await confirmRetranscribe();
    if (!confirmed) {
      return;
    }

    const result = await fsSyncCommands.audioPath(sessionId);
    if (result.status === "error") {
      showTransientToast({
        id: `transcript-regenerate-audio-missing-${sessionId}`,
        description: "Recording not found. It may have been deleted.",
        variant: "error",
      });
      return;
    }

    const audioPath = result.data;

    try {
      await runBatch(audioPath);
      await getEnhancerService()?.queueAutoEnhanceIfSummaryEmpty(sessionId);
    } catch (error) {
      if (isStoppedTranscriptionError(error)) {
        return;
      }
      const msg = error instanceof Error ? error.message : String(error);
      handleBatchFailed(sessionId, msg);
    }
  }, [handleBatchFailed, runBatch, sessionId]);
}

// Like useRegenerateTranscript, but with an explicitly chosen local model
// (the "Re-transcribe with…" picker) instead of the configured final model.
// The chosen model comes from the local-stt supported-model catalog, so the
// local server is swapped to it for the batch pass and the live server is
// restored afterwards.
export function useRegenerateTranscriptWithModel(sessionId: string) {
  const runBatch = useRunBatch(sessionId);
  const handleBatchFailed = useListener((state) => state.handleBatchFailed);
  const { conn } = useSTTConnection();

  return useCallback(
    async (model: string) => {
      const confirmed = await confirmRetranscribe();
      if (!confirmed) {
        return;
      }

      const result = await fsSyncCommands.audioPath(sessionId);
      if (result.status === "error") {
        showTransientToast({
          id: `transcript-regenerate-audio-missing-${sessionId}`,
          description: "Recording not found. It may have been deleted.",
          variant: "error",
        });
        return;
      }

      const target = await resolveFinalBatchTarget({
        provider: "hyprnote",
        liveModel: conn?.provider === "hyprnote" ? conn.model : null,
        finalModel: model,
      });
      if (!target) {
        showTransientToast({
          id: `transcript-regenerate-model-unavailable-${sessionId}`,
          description:
            "The selected model is not available. It may need to be downloaded again.",
          variant: "error",
        });
        return;
      }

      try {
        await runBatch(result.data, {
          model: target.model,
          baseUrl: target.baseUrl,
          provider: "hyprnote",
        });
        await getEnhancerService()?.queueAutoEnhanceIfSummaryEmpty(sessionId);
      } catch (error) {
        if (isStoppedTranscriptionError(error)) {
          return;
        }
        const msg = error instanceof Error ? error.message : String(error);
        handleBatchFailed(sessionId, msg);
      } finally {
        // useRunBatch only restores servers for targets it resolved itself;
        // this explicit target is ours to restore.
        await target.restore();
      }
    },
    [conn?.model, conn?.provider, handleBatchFailed, runBatch, sessionId],
  );
}

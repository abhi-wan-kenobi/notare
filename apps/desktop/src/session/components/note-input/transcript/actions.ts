import { ask } from "@tauri-apps/plugin-dialog";
import { useCallback } from "react";

import { commands as fsSyncCommands } from "@hypr/plugin-fs-sync";

import { getEnhancerService } from "~/services/enhancer";
import { showTransientToast } from "~/sidebar/toast/transient";
import { useListener } from "~/stt/contexts";
import { isStoppedTranscriptionError, useRunBatch } from "~/stt/useRunBatch";

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

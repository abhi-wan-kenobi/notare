import { useLingui } from "@lingui/react/macro";
import { useCallback } from "react";
import { useHotkeys } from "react-hotkeys-hook";

import { sonnerToast } from "@hypr/ui/components/ui/toast";

import {
  COPY_FULL_LIVE_TRANSCRIPT_HOTKEY,
  COPY_LATEST_LIVE_CHUNK_HOTKEY,
  copyLiveTranscript,
  isLiveTranscriptCopyAvailable,
  type LiveTranscriptCopyKind,
} from "./live-transcript-clipboard";

/**
 * In-app shortcuts to copy the live transcript while recording:
 * - mod+shift+c copies the latest live chunk (most recent utterance)
 * - mod+shift+f copies the full transcript so far
 *
 * They only do anything while a live session is active, so they never
 * shadow other behavior outside of recording. These are app-focused
 * shortcuts; a system-wide hotkey is a documented follow-up.
 */
export function useLiveTranscriptCopyShortcuts() {
  const { t } = useLingui();

  const handleCopy = useCallback(
    async (kind: LiveTranscriptCopyKind) => {
      const result = await copyLiveTranscript(kind);
      if (result === "inactive") {
        return;
      }

      if (result === "empty") {
        sonnerToast.info(t`Nothing transcribed yet`);
        return;
      }

      sonnerToast.success(
        kind === "latest"
          ? t`Latest transcript chunk copied`
          : t`Live transcript copied`,
      );
    },
    [t],
  );

  const makeHandler = useCallback(
    (kind: LiveTranscriptCopyKind) => (event: KeyboardEvent) => {
      if (!isLiveTranscriptCopyAvailable()) {
        return;
      }

      event.preventDefault();
      void handleCopy(kind);
    },
    [handleCopy],
  );

  useHotkeys(
    COPY_LATEST_LIVE_CHUNK_HOTKEY,
    makeHandler("latest"),
    {
      enableOnFormTags: true,
      enableOnContentEditable: true,
    },
    [makeHandler],
  );

  useHotkeys(
    COPY_FULL_LIVE_TRANSCRIPT_HOTKEY,
    makeHandler("full"),
    {
      enableOnFormTags: true,
      enableOnContentEditable: true,
    },
    [makeHandler],
  );
}

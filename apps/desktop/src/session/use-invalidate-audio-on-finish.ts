import { useQueryClient } from "@tanstack/react-query";
import { useEffect, useRef } from "react";

import { type SessionMode } from "~/store/zustand/listener";
import { useListener } from "~/stt/contexts";

// `useAudioExists` resolves the `["audio", sessionId, "exist"]` query once on
// mount and is never refetched on its own, so when a recording finishes writing
// the audio file the player stays hidden until a remount. Re-querying the file's
// existence (and its URL) the moment the session leaves an active/finalizing/
// batch state makes the player appear without leaving the tab.
export function useInvalidateAudioOnRecordingFinish(sessionId: string) {
  const queryClient = useQueryClient();
  const sessionMode = useListener((state) => state.getSessionMode(sessionId));
  const previousModeRef = useRef<SessionMode | null>(null);

  useEffect(() => {
    const previousMode = previousModeRef.current;
    previousModeRef.current = sessionMode;

    if (previousMode === null) {
      return;
    }

    const wasCapturing =
      previousMode === "active" ||
      previousMode === "finalizing" ||
      previousMode === "running_batch";

    if (wasCapturing && sessionMode === "inactive") {
      void queryClient.invalidateQueries({
        queryKey: ["audio", sessionId, "exist"],
      });
      void queryClient.invalidateQueries({
        queryKey: ["audio", sessionId, "url"],
      });
    }
  }, [sessionMode, sessionId, queryClient]);
}

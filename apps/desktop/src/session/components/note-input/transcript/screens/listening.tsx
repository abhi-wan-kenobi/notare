import { useLingui } from "@lingui/react/macro";
import { AudioLinesIcon } from "lucide-react";

import { Spinner } from "@hypr/ui/components/ui/spinner";

import { getLiveTranscriptShortcutHints } from "~/stt/live-transcript-clipboard";

export function TranscriptListeningState({
  status,
}: {
  status: "listening" | "finalizing";
}) {
  const { t } = useLingui();
  const isFinalizing = status === "finalizing";
  const shortcutHints = getLiveTranscriptShortcutHints();

  return (
    <div className="text-muted-foreground flex h-full flex-col items-center justify-center gap-3">
      {isFinalizing ? (
        <Spinner size={28} />
      ) : (
        <AudioLinesIcon className="h-8 w-8" />
      )}
      <div className="flex max-w-sm flex-col items-center gap-1 text-center">
        <p className="text-muted-foreground text-sm">
          {isFinalizing ? "Finalizing transcript..." : "Listening..."}
        </p>
        <p className="text-muted-foreground text-xs">
          {isFinalizing
            ? "Transcript is still being written."
            : "Transcript will appear here when the first segment arrives."}
        </p>
        {!isFinalizing && (
          <p className="text-muted-foreground/80 text-xs">
            {t`Copy live text: ${shortcutHints.latest} (latest chunk) · ${shortcutHints.full} (all so far)`}
          </p>
        )}
      </div>
    </div>
  );
}

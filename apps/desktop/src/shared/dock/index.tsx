import { useLingui } from "@lingui/react/macro";
import { MicIcon, SparklesIcon } from "lucide-react";
import { useCallback, useState } from "react";

import { Kbd } from "@hypr/ui/components/ui/kbd";
import { cn } from "@hypr/utils";

import { useShell } from "~/contexts/shell";
import { useNewNoteAndListen } from "~/shared/useNewNote";
import { useListener } from "~/stt/contexts";
import { useStartListening } from "~/stt/useStartListening";
import { useSTTConnection } from "~/stt/useSTTConnection";

/**
 * The floating dock: a mic/record segment and an "ask anywhere" segment
 * that opens the inspector. Mounted for the home surface and open sessions
 * (shell.css `.dock`), floating above the bottom edge of the reading
 * column.
 */
export function Dock({ sessionId }: { sessionId?: string }) {
  return (
    <div className="pointer-events-none absolute right-0 bottom-3 left-0 z-20 flex justify-center px-4">
      <div className="border-border bg-card pointer-events-auto flex h-12 w-full max-w-[700px] items-center rounded-2xl border p-1 shadow-md">
        <DockRecordSegment sessionId={sessionId} />
        <DockAskSegment />
      </div>
    </div>
  );
}

function DockRecordSegment({ sessionId }: { sessionId?: string }) {
  const { t } = useLingui();
  const { status, seconds, liveSessionId } = useListener((state) => ({
    status: state.live.status,
    seconds: state.live.seconds,
    liveSessionId: state.live.sessionId,
  }));
  const stop = useListener((state) => state.stop);
  const { isLocalModel, local } = useSTTConnection();
  const liveReady = isLocalModel && local.data?.status === "ready";

  const isThisDockRecording =
    (status === "active" || status === "finalizing") &&
    (sessionId ? liveSessionId === sessionId : liveSessionId !== null);

  const startListeningThisSession = useStartListening(sessionId ?? "");
  const startNewNoteAndListen = useNewNoteAndListen();

  const handleClick = useCallback(() => {
    if (isThisDockRecording) {
      stop();
      return;
    }
    if (sessionId) {
      void startListeningThisSession();
    } else {
      startNewNoteAndListen();
    }
  }, [
    isThisDockRecording,
    sessionId,
    startListeningThisSession,
    startNewNoteAndListen,
    stop,
  ]);

  return (
    <button
      type="button"
      onClick={handleClick}
      disabled={!isThisDockRecording && !liveReady}
      aria-label={isThisDockRecording ? t`Stop recording` : t`Start recording`}
      className={cn([
        "border-border/70 text-muted-foreground flex h-full shrink-0 items-center gap-2 border-r px-3.5 text-xs whitespace-nowrap",
        "disabled:opacity-50",
        isThisDockRecording && "text-foreground",
      ])}
    >
      <MicIcon
        size={15}
        className={isThisDockRecording ? "text-lime" : undefined}
      />
      <DockBars active={isThisDockRecording} />
      <span className="tabular-nums">
        {isThisDockRecording ? formatElapsed(seconds) : t`Ready`}
      </span>
    </button>
  );
}

function DockBars({ active }: { active: boolean }) {
  return (
    <span className="flex h-4 items-center gap-0.5">
      {[0, 1, 2].map((i) => (
        <span
          key={i}
          aria-hidden="true"
          className={cn([
            "bg-lime w-0.5 rounded-sm",
            active ? "animate-dock-bar h-2.5" : "h-1.5 opacity-40",
          ])}
          style={active ? { animationDelay: `${i * 0.2}s` } : undefined}
        />
      ))}
    </span>
  );
}

function DockAskSegment() {
  const { t } = useLingui();
  const { inspector } = useShell();
  const [value, setValue] = useState("");

  const handleSubmit = useCallback(
    (event: React.FormEvent) => {
      event.preventDefault();
      const prompt = value.trim();
      if (!prompt) {
        return;
      }
      inspector.openWith(prompt);
      setValue("");
    },
    [inspector, value],
  );

  return (
    <form
      onSubmit={handleSubmit}
      className="flex min-w-0 flex-1 items-center gap-2 px-2.5"
    >
      <SparklesIcon size={14} className="text-muted-foreground shrink-0" />
      <input
        type="text"
        value={value}
        onChange={(event) => setValue(event.target.value)}
        placeholder={t`Ask anything across your meetings…`}
        className="placeholder:text-tertiary min-w-0 flex-1 bg-transparent text-xs outline-none"
      />
      <Kbd className="text-[10px]">/</Kbd>
    </form>
  );
}

function formatElapsed(totalSeconds: number): string {
  const safeSeconds = Math.max(0, Math.floor(totalSeconds));
  const minutes = Math.floor(safeSeconds / 60);
  const seconds = safeSeconds % 60;
  return `${String(minutes).padStart(2, "0")}:${String(seconds).padStart(2, "0")}`;
}

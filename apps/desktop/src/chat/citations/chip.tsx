import { FileText } from "lucide-react";
import type React from "react";

import { cn } from "@hypr/utils";

import { requestSeek } from "~/audio-player/pending-seek";
import { formatTimestamp } from "~/chat/citations/parser";
import { useSessionSummaries } from "~/session/queries";
import { useTabs } from "~/store/zustand/tabs";

export interface CitationChipProps {
  sessionId: string;
  ms: number;
  raw?: string;
  className?: string;
  onNavigate?: (sessionId: string, ms: number) => void;
}

export function CitationChip({
  sessionId,
  ms,
  raw,
  className,
  onNavigate,
}: CitationChipProps) {
  const sessions = useSessionSummaries();
  const session = sessions.find((s) => s.id === sessionId);

  // If unknown session id, render as plain text without crashing
  if (!session) {
    return (
      <span className="text-muted-foreground font-mono text-xs">
        {raw ?? `[[src:${sessionId}@${ms}]]`}
      </span>
    );
  }

  const title = session.title?.trim() || "Untitled meeting";
  const formattedTime = formatTimestamp(ms);

  const handleClick = (e: React.MouseEvent) => {
    e.preventDefault();
    e.stopPropagation();

    if (onNavigate) {
      onNavigate(sessionId, ms);
      return;
    }

    requestSeek(sessionId, ms);
    useTabs.getState().openNew({ type: "sessions", id: sessionId });
  };

  return (
    <button
      type="button"
      data-citation-chip
      data-session-id={sessionId}
      data-timestamp-ms={ms}
      onClick={handleClick}
      title={`Jump to "${title}" at ${formattedTime}`}
      className={cn([
        "group mx-1 inline-flex max-w-64 min-w-0 items-center gap-1.5 align-middle select-none",
        "border-border/70 bg-card/90 text-foreground rounded-md border px-2 py-0.5 text-xs font-medium",
        "hover:border-border hover:bg-accent/40 active:bg-accent/60 cursor-pointer shadow-xs transition-colors",
        className,
      ])}
    >
      <FileText className="text-muted-foreground group-hover:text-foreground size-3 shrink-0 transition-opacity" />
      <span className="truncate">{title}</span>
      <span className="text-muted-foreground shrink-0 font-mono text-[10px] tabular-nums">
        {formattedTime}
      </span>
    </button>
  );
}

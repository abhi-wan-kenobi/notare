import { FileText } from "lucide-react";
import type React from "react";

import { cn } from "@hypr/utils";

import { requestSeek } from "~/audio-player/pending-seek";
import { formatTimestamp } from "~/chat/citations/parser";
import { useSessionSummaries } from "~/session/queries";
import { useTabs } from "~/store/zustand/tabs";

export interface CitationChipProps {
  sessionId?: string;
  ms: number;
  raw?: string;
  className?: string;
  variant?: "default" | "timestamp";
  onNavigate?: (sessionId: string, ms: number) => void;
}

export function CitationChip({
  sessionId,
  ms,
  raw,
  className,
  variant,
  onNavigate,
}: CitationChipProps) {
  const sessions = useSessionSummaries();
  const currentTab = useTabs((state) => state.currentTab);
  const currentSessionId =
    currentTab?.type === "sessions" ? currentTab.id : undefined;
  const effectiveSessionId = sessionId ?? currentSessionId;
  const session = effectiveSessionId
    ? sessions.find((s) => s.id === effectiveSessionId)
    : undefined;

  // If unknown session id in default mode, render as plain text without crashing
  if (sessionId && !session && variant !== "timestamp") {
    return (
      <span className="text-muted-foreground font-mono text-xs">
        {raw ?? `[[src:${sessionId}@${ms}]]`}
      </span>
    );
  }

  const title = session?.title?.trim() || "Untitled meeting";
  const formattedTime = formatTimestamp(ms);

  const handleClick = (e: React.MouseEvent) => {
    e.preventDefault();
    e.stopPropagation();

    if (onNavigate && effectiveSessionId) {
      onNavigate(effectiveSessionId, ms);
      return;
    }

    try {
      const win = window as unknown as {
        __sessionStore?: { seekTo?: (ms: number) => void };
        sessionStore?: { seekTo?: (ms: number) => void };
      };
      if (typeof win.__sessionStore?.seekTo === "function") {
        win.__sessionStore.seekTo(ms);
      } else if (typeof win.sessionStore?.seekTo === "function") {
        win.sessionStore.seekTo(ms);
      }
    } catch {
      // ignore
    }

    if (typeof window !== "undefined") {
      window.dispatchEvent(
        new CustomEvent("notare:seek", {
          detail: { ms, sessionId: effectiveSessionId },
        }),
      );
    }

    if (effectiveSessionId) {
      requestSeek(effectiveSessionId, ms);
      const tab = useTabs.getState().currentTab;
      if (tab?.type !== "sessions" || tab.id !== effectiveSessionId) {
        useTabs
          .getState()
          .openNew({ type: "sessions", id: effectiveSessionId });
      }
    }
  };

  if (variant === "timestamp") {
    return (
      <button
        type="button"
        data-citation-chip
        data-session-id={effectiveSessionId}
        data-timestamp-ms={ms}
        onClick={handleClick}
        title={
          session?.title
            ? `${session.title} · ${formattedTime}`
            : `Jump to ${formattedTime}`
        }
        className={cn([
          "group mx-1 inline-flex items-center gap-1 align-middle select-none",
          "border-border/70 bg-card/90 text-foreground rounded-md border px-1.5 py-0.5 text-xs font-medium",
          "hover:border-border hover:bg-accent/40 active:bg-accent/60 cursor-pointer shadow-xs transition-colors",
          className,
        ])}
      >
        <span className="text-accent/80 shrink-0 font-mono text-xs">▸</span>
        <span className="font-mono text-[11px] tabular-nums">
          {formattedTime}
        </span>
      </button>
    );
  }
  return (
    <button
      type="button"
      data-citation-chip
      data-session-id={effectiveSessionId}
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

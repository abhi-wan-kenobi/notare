import {
  ChevronDown,
  ChevronRight,
  RefreshCw,
  Sparkles,
  X,
} from "lucide-react";
import { useCallback, useState } from "react";

import { Button } from "@hypr/ui/components/ui/button";
import { cn } from "@hypr/utils";

import { regenerateBriefForEvent } from "./generator";
import { dismissBrief, useBriefForEvent, useBriefForSession } from "./storage";
import type { SessionBrief } from "./types";

import { CitationChip } from "~/chat/citations/chip";
import { parseCitationSegments } from "~/chat/citations/parser";
import { useTaskModel } from "~/services/llm-router";

export function BriefCard({
  eventId,
  sessionId,
  initialBrief,
  className,
}: {
  eventId?: string;
  sessionId?: string;
  initialBrief?: SessionBrief | null;
  className?: string;
}) {
  const briefFromEvent = useBriefForEvent(eventId);
  const briefFromSession = useBriefForSession(sessionId);
  const effectiveInitial = initialBrief?.dismissed_at ? null : initialBrief;
  const brief = effectiveInitial ?? briefFromEvent ?? briefFromSession;

  const [isOpen, setIsOpen] = useState(true);
  const [isRegenerating, setIsRegenerating] = useState(false);
  const [isDismissed, setIsDismissed] = useState(false);

  const { model, resolution } = useTaskModel("brief");
  const modelName =
    resolution.status === "ok"
      ? `${resolution.providerId}/${resolution.modelId}`
      : "notes-scope";

  const handleDismiss = useCallback(async () => {
    if (!brief) return;
    setIsDismissed(true);
    try {
      await dismissBrief(brief.id);
    } catch (e) {
      setIsDismissed(false);
      console.error("[briefs] failed to dismiss brief", e);
    }
  }, [brief]);

  const handleRegenerate = useCallback(async () => {
    const targetEventId = brief?.event_id || eventId;
    if (!targetEventId || !model || isRegenerating) return;

    setIsRegenerating(true);
    try {
      await regenerateBriefForEvent(targetEventId, model, modelName);
    } finally {
      setIsRegenerating(false);
    }
  }, [brief?.event_id, eventId, model, modelName, isRegenerating]);

  if (!brief || isDismissed) {
    return null;
  }

  return (
    <div
      data-brief-card
      className={cn([
        "border-border/70 bg-card/60 dark:bg-card/40 flex flex-col rounded-lg border text-sm",
        className,
      ])}
    >
      <div className="flex items-center justify-between px-3 py-2">
        <button
          type="button"
          data-brief-toggle
          onClick={() => setIsOpen((prev) => !prev)}
          className="hover:text-foreground text-foreground/90 flex items-center gap-1.5 font-medium transition-colors"
          aria-expanded={isOpen}
        >
          {isOpen ? (
            <ChevronDown className="text-muted-foreground size-4 shrink-0" />
          ) : (
            <ChevronRight className="text-muted-foreground size-4 shrink-0" />
          )}
          <Sparkles className="size-3.5 shrink-0 text-amber-500 dark:text-amber-400" />
          <span>Brief</span>
          {isRegenerating && (
            <RefreshCw className="text-muted-foreground size-3 animate-spin" />
          )}
        </button>

        <div className="flex items-center gap-1">
          <Button
            type="button"
            variant="ghost"
            size="icon"
            data-brief-regenerate
            onClick={handleRegenerate}
            disabled={!model || isRegenerating}
            title="Regenerate brief"
            className="text-muted-foreground hover:text-foreground size-6 rounded-md"
          >
            <RefreshCw
              className={cn(["size-3", isRegenerating && "animate-spin"])}
            />
          </Button>
          <Button
            type="button"
            variant="ghost"
            size="icon"
            data-brief-dismiss
            onClick={handleDismiss}
            title="Dismiss brief"
            className="text-muted-foreground hover:text-foreground size-6 rounded-md"
          >
            <X className="size-3.5" />
          </Button>
        </div>
      </div>

      {isOpen && (
        <div
          data-brief-content
          className="border-border/50 text-foreground/90 border-t px-3 py-2.5 text-xs leading-relaxed"
        >
          <BriefContentRenderer content={brief.content_md} />
        </div>
      )}
    </div>
  );
}

export function BriefContentRenderer({ content }: { content: string }) {
  const paragraphs = content.split("\n\n");

  return (
    <div className="flex flex-col gap-2">
      {paragraphs.map((para, paraIdx) => {
        const lines = para.split("\n");
        return (
          <div key={`p-${paraIdx}`} className="flex flex-col gap-1">
            {lines.map((line, lineIdx) => {
              const isHeading = line.startsWith("#");
              const isBullet =
                line.trimStart().startsWith("- ") ||
                line.trimStart().startsWith("* ");
              const cleanLine = isHeading
                ? line.replace(/^#+\s*/, "")
                : isBullet
                  ? line.replace(/^(\s*[-*]\s*)/, "")
                  : line;

              const segments = parseCitationSegments(cleanLine);

              if (isHeading) {
                return (
                  <h4
                    key={`l-${lineIdx}`}
                    className="text-foreground mt-1 font-semibold first:mt-0"
                  >
                    {segments.map((segment, segIdx) => {
                      if (segment.type === "citation") {
                        return (
                          <CitationChip
                            key={`c-${segIdx}`}
                            sessionId={segment.sessionId}
                            ms={segment.ms}
                            raw={segment.raw}
                          />
                        );
                      }
                      return <span key={`t-${segIdx}`}>{segment.text}</span>;
                    })}
                  </h4>
                );
              }

              return (
                <div
                  key={`l-${lineIdx}`}
                  className={cn([
                    "leading-relaxed",
                    isBullet && "flex items-start gap-1.5 pl-1.5",
                  ])}
                >
                  {isBullet && (
                    <span className="text-muted-foreground select-none">•</span>
                  )}
                  <span className="flex-1">
                    {segments.map((segment, segIdx) => {
                      if (segment.type === "citation") {
                        return (
                          <CitationChip
                            key={`c-${segIdx}`}
                            sessionId={segment.sessionId}
                            ms={segment.ms}
                            raw={segment.raw}
                          />
                        );
                      }
                      return <span key={`t-${segIdx}`}>{segment.text}</span>;
                    })}
                  </span>
                </div>
              );
            })}
          </div>
        );
      })}
    </div>
  );
}

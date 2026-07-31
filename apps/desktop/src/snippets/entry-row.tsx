import { Trans, useLingui } from "@lingui/react/macro";
import {
  ChevronDownIcon,
  ChevronUpIcon,
  CopyIcon,
  PinIcon,
  PinOffIcon,
  TextCursorInputIcon,
  Trash2Icon,
} from "lucide-react";
import { useState } from "react";

import { Badge } from "@hypr/ui/components/ui/badge";
import { Button } from "@hypr/ui/components/ui/button";
import { cn, format } from "@hypr/utils";

import type { DictationHistoryEntry } from "~/dictation/history";

/**
 * One dictation history row: cleaned text, time/source/model metadata, and
 * the four snippet actions (copy / insert at cursor / pin / delete). The
 * bucket header above each day group already carries the date, so this row
 * only shows the time-of-day.
 */
export function SnippetEntryRow({
  entry,
  onCopy,
  onInsert,
  onTogglePinned,
  onDelete,
}: {
  entry: DictationHistoryEntry;
  onCopy: (entry: DictationHistoryEntry) => void;
  onInsert: (entry: DictationHistoryEntry) => void;
  onTogglePinned: (entry: DictationHistoryEntry) => void;
  onDelete: (entry: DictationHistoryEntry) => void;
}) {
  const { t } = useLingui();
  const [showRaw, setShowRaw] = useState(false);

  const isDiscarded = entry.status === "discarded";
  const hasRawDiff = entry.rawText !== null && entry.rawText !== entry.text;
  const createdAt = new Date(entry.createdAt);
  const hasValidCreatedAt = !Number.isNaN(createdAt.getTime());

  return (
    <li
      data-testid="snippet-entry"
      data-entry-id={entry.id}
      data-discarded={isDiscarded || undefined}
      className={cn([
        "group flex flex-col gap-1.5 rounded-lg border p-3",
        "border-border",
        isDiscarded ? "bg-muted/40 opacity-70" : "hover:bg-accent/20",
      ])}
    >
      <div className="flex items-start justify-between gap-3">
        <p
          className={cn([
            "min-w-0 flex-1 text-sm whitespace-pre-wrap",
            isDiscarded && "text-muted-foreground italic",
          ])}
        >
          {entry.text}
        </p>
        <div className="flex shrink-0 items-center gap-0.5">
          <Button
            variant="ghost"
            size="icon"
            aria-label={t`Copy`}
            onClick={() => onCopy(entry)}
          >
            <CopyIcon className="size-4" />
          </Button>
          <Button
            variant="ghost"
            size="icon"
            aria-label={t`Insert at cursor`}
            onClick={() => onInsert(entry)}
          >
            <TextCursorInputIcon className="size-4" />
          </Button>
          <Button
            variant="ghost"
            size="icon"
            aria-label={entry.pinned ? t`Unpin` : t`Pin`}
            aria-pressed={entry.pinned}
            onClick={() => onTogglePinned(entry)}
          >
            {entry.pinned ? (
              <PinOffIcon className="size-4" />
            ) : (
              <PinIcon className="size-4" />
            )}
          </Button>
          <Button
            variant="ghost"
            size="icon"
            aria-label={t`Delete`}
            onClick={() => onDelete(entry)}
          >
            <Trash2Icon className="size-4" />
          </Button>
        </div>
      </div>

      <div className="text-muted-foreground flex flex-wrap items-center gap-x-2 gap-y-1 text-xs">
        {hasValidCreatedAt ? (
          <span title={entry.createdAt}>{format(createdAt, "p")}</span>
        ) : null}
        <Badge variant={entry.source === "meeting" ? "secondary" : "outline"}>
          {entry.source === "meeting" ? (
            <Trans>Meeting</Trans>
          ) : (
            <Trans>Dictation</Trans>
          )}
        </Badge>
        {isDiscarded ? (
          <Badge variant="destructive">
            <Trans>Discarded</Trans>
          </Badge>
        ) : null}
        {entry.model ? <span>{entry.model}</span> : null}
        {entry.durationMs !== null ? (
          <span>{formatDuration(entry.durationMs)}</span>
        ) : null}
        {hasRawDiff ? (
          <button
            type="button"
            aria-expanded={showRaw}
            onClick={() => setShowRaw((prev) => !prev)}
            className="hover:text-foreground inline-flex items-center gap-0.5 transition-colors"
          >
            {showRaw ? (
              <ChevronUpIcon className="size-3" />
            ) : (
              <ChevronDownIcon className="size-3" />
            )}
            <Trans>Raw transcript</Trans>
          </button>
        ) : null}
      </div>

      {hasRawDiff && showRaw ? (
        <p className="text-muted-foreground border-border/60 rounded-md border border-dashed p-2 text-xs whitespace-pre-wrap">
          {entry.rawText}
        </p>
      ) : null}
    </li>
  );
}

/** `8000` -> "8s"; `65000` -> "1m 5s"; `7_265_000` -> "2h 1m". */
function formatDuration(durationMs: number): string {
  const totalSeconds = Math.max(0, Math.round(durationMs / 1000));
  const hours = Math.floor(totalSeconds / 3600);
  const minutes = Math.floor((totalSeconds % 3600) / 60);
  const seconds = totalSeconds % 60;
  if (hours > 0) {
    return `${hours}h ${minutes}m`;
  }
  return minutes > 0 ? `${minutes}m ${seconds}s` : `${seconds}s`;
}

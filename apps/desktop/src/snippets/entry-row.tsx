import { Trans, useLingui } from "@lingui/react/macro";
import {
  CheckIcon,
  ChevronDownIcon,
  ChevronUpIcon,
  CopyIcon,
  PencilIcon,
  PinIcon,
  PinOffIcon,
  TextCursorInputIcon,
  Trash2Icon,
  XIcon,
} from "lucide-react";
import { useState } from "react";

import { Badge } from "@hypr/ui/components/ui/badge";
import { Button } from "@hypr/ui/components/ui/button";
import { cn, format } from "@hypr/utils";

import type { DictationHistoryEntry } from "~/dictation/history";

/**
 * One dictation history row: cleaned text, time/source/model metadata, and
 * the snippet actions (copy / insert at cursor / edit / pin / delete). The
 * bucket header above each day group already carries the date, so this row
 * only shows the time-of-day.
 *
 * Editing swaps the text into a textarea and the action row into Save/Cancel
 * - the other row chrome (metadata, raw-transcript toggle) hides while
 * editing since none of it applies to an unsaved draft.
 */
export function SnippetEntryRow({
  entry,
  onCopy,
  onInsert,
  onTogglePinned,
  onDelete,
  onEditSave,
}: {
  entry: DictationHistoryEntry;
  onCopy: (entry: DictationHistoryEntry) => void;
  onInsert: (entry: DictationHistoryEntry) => void;
  onTogglePinned: (entry: DictationHistoryEntry) => void;
  onDelete: (entry: DictationHistoryEntry) => void;
  onEditSave: (entry: DictationHistoryEntry, newText: string) => void;
}) {
  const { t } = useLingui();
  const [showRaw, setShowRaw] = useState(false);
  const [isEditing, setIsEditing] = useState(false);
  const [draftText, setDraftText] = useState(entry.text);

  const isDiscarded = entry.status === "discarded";
  const hasRawDiff = entry.rawText !== null && entry.rawText !== entry.text;
  const createdAt = new Date(entry.createdAt);
  const hasValidCreatedAt = !Number.isNaN(createdAt.getTime());

  const trimmedDraft = draftText.trim();
  const canSaveEdit = trimmedDraft.length > 0 && trimmedDraft !== entry.text;

  const startEdit = () => {
    setDraftText(entry.text);
    setIsEditing(true);
  };
  const cancelEdit = () => {
    setIsEditing(false);
    setDraftText(entry.text);
  };
  const saveEdit = () => {
    if (!canSaveEdit) {
      setIsEditing(false);
      return;
    }
    onEditSave(entry, trimmedDraft);
    setIsEditing(false);
  };

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
        {isEditing ? (
          <textarea
            data-testid="snippet-entry-edit-textarea"
            aria-label={t`Edit snippet text`}
            value={draftText}
            onChange={(event) => setDraftText(event.target.value)}
            rows={3}
            // eslint-disable-next-line jsx-a11y/no-autofocus
            autoFocus
            className={cn([
              "border-border bg-background min-w-0 flex-1 resize-y rounded-md border p-2 text-sm",
              "focus-visible:ring-ring focus-visible:ring-1 focus-visible:outline-hidden",
            ])}
          />
        ) : (
          <p
            className={cn([
              "min-w-0 flex-1 text-sm whitespace-pre-wrap",
              isDiscarded && "text-muted-foreground italic",
            ])}
          >
            {entry.text}
          </p>
        )}
        <div className="flex shrink-0 items-center gap-0.5">
          {isEditing ? (
            <>
              <Button
                variant="ghost"
                size="icon"
                aria-label={t`Save`}
                disabled={!canSaveEdit}
                onClick={saveEdit}
              >
                <CheckIcon className="size-4" />
              </Button>
              <Button
                variant="ghost"
                size="icon"
                aria-label={t`Cancel`}
                onClick={cancelEdit}
              >
                <XIcon className="size-4" />
              </Button>
            </>
          ) : (
            <>
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
                aria-label={t`Edit`}
                onClick={startEdit}
              >
                <PencilIcon className="size-4" />
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
            </>
          )}
        </div>
      </div>

      {isEditing ? null : (
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
      )}

      {!isEditing && hasRawDiff && showRaw ? (
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

/**
 * Per-session action-items checklist panel (WS-C PR18).
 *
 * Reads the session's `action_items` rows live (see `./queries`), renders each
 * as a toggleable checklist row with confidence / owner / due / priority
 * affordances, and offers an "Extract action items" action that runs the WS-C
 * extraction pipeline against the session transcript and persists the gated
 * results.
 *
 * Mounted as a collapsible section beneath the note editor so it is additive
 * and never displaces the primary editing surface.
 */

import { Trans, useLingui } from "@lingui/react/macro";
import {
  ChevronDownIcon,
  ChevronRightIcon,
  ClockIcon,
  CornerDownRightIcon,
  ListChecksIcon,
  SparklesIcon,
  UserIcon,
} from "lucide-react";
import { useCallback, useMemo, useState } from "react";

import { Badge } from "@hypr/ui/components/ui/badge";
import { Button } from "@hypr/ui/components/ui/button";
import { Checkbox } from "@hypr/ui/components/ui/checkbox";
import { Spinner } from "@hypr/ui/components/ui/spinner";
import { sonnerToast } from "@hypr/ui/components/ui/toast";
import { cn } from "@hypr/utils";

import { buildExtractionInput } from "./extraction-input";
import {
  insertSessionActionItems,
  loadSessionActionItemCount,
  type SessionActionItemRecord,
  setActionItemStatus,
  useSessionActionItems,
} from "./queries";

import { extractActionItems } from "~/services/action-items/extract";
import { checkStructuredCapability } from "~/services/action-items/structured-capability";
import { useTaskModel } from "~/services/llm-router";
import { loadSessionContentSnapshot } from "~/session/content-queries";
import { useOwnerUserId } from "~/shared/owner-user";
import { DEFAULT_USER_ID } from "~/shared/utils";
import { useTabs } from "~/store/zustand/tabs";

export function ActionItemsPanel({ sessionId }: { sessionId: string }) {
  const { t } = useLingui();
  const [expanded, setExpanded] = useState(false);
  const { items, isLoading } = useSessionActionItems(sessionId);
  const ownerUserId = useOwnerUserId() || DEFAULT_USER_ID;

  const { model, resolution, target } = useTaskModel("action_items");
  const [isExtracting, setIsExtracting] = useState(false);
  const [extractError, setExtractError] = useState<string | null>(null);

  // Owner labels are only known for items extracted this session (speaker ids
  // are not human-readable on their own). We cache the extraction-time map so
  // freshly-extracted rows show a name immediately; older rows simply hide the
  // owner chip — a documented acceptable edge case for this PR.
  const [ownerLabels, setOwnerLabels] = useState<Map<string, string>>(
    () => new Map(),
  );

  const resolutionMessage = useMemo(() => {
    if (resolution.status === "ok") {
      return null;
    }
    switch (resolution.reason) {
      case "caps_unmet":
        return t`Select a capable model`;
      case "cloud_not_opted_in":
        return t`Select a cloud model to use`;
      case "no_provider":
      default:
        return t`Enable a local or cloud model`;
    }
  }, [resolution, t]);

  const handleExtract = useCallback(async () => {
    setExtractError(null);

    if (resolution.status !== "ok" || !model || !target) {
      setExpanded(true);
      setExtractError(resolutionMessage);
      return;
    }

    setIsExtracting(true);
    setExpanded(true);
    try {
      // Runtime PG gate: refuse a BYO endpoint that can't actually emit
      // structured output (ollama is exempt — it uses the native format path).
      const capability = await checkStructuredCapability(target);
      if (!capability.ok) {
        setExtractError(
          t`This model can't produce structured output. Pick a different model.`,
        );
        return;
      }

      const snapshot = await loadSessionContentSnapshot(sessionId);
      if (!snapshot) {
        setExtractError(t`Session content is not available yet.`);
        return;
      }

      const { transcript, words, roster, labelBySpeakerId } =
        buildExtractionInput(snapshot);
      if (!transcript.trim() || words.length === 0) {
        setExtractError(t`No transcript to extract from yet.`);
        return;
      }

      const meetingDate = new Date(snapshot.createdAt || Date.now());
      const result = await extractActionItems(
        model,
        {
          transcript,
          words,
          roster,
          meetingDate: Number.isNaN(meetingDate.getTime())
            ? new Date()
            : meetingDate,
        },
        // ollama routes to the native `format` endpoint via this target.
        { target },
      );

      setOwnerLabels(new Map(labelBySpeakerId));

      const startOrder = await loadSessionActionItemCount(sessionId);
      await insertSessionActionItems(
        sessionId,
        ownerUserId,
        result.kept,
        startOrder,
      );

      if (result.kept.length === 0) {
        sonnerToast.info(t`No action items found in this meeting.`);
      }
    } catch (error) {
      console.error("[action-items] extraction failed", error);
      setExtractError(t`Extraction failed. Please try again.`);
    } finally {
      setIsExtracting(false);
    }
  }, [model, target, ownerUserId, resolution, resolutionMessage, sessionId, t]);

  const handleToggle = useCallback(
    (item: SessionActionItemRecord) => {
      const nextStatus = item.status === "done" ? "todo" : "done";
      void setActionItemStatus(item.id, nextStatus, ownerUserId).catch(
        (error) => {
          console.error("[action-items] failed to toggle status", error);
        },
      );
    },
    [ownerUserId],
  );

  const count = items.length;

  return (
    <section
      data-action-items-panel
      className="border-border/70 bg-card/40 shrink-0 overflow-hidden rounded-[18px] border"
    >
      <header className="flex items-center gap-2 px-3 py-2">
        <button
          type="button"
          onClick={() => setExpanded((value) => !value)}
          aria-expanded={expanded}
          className="text-foreground flex min-w-0 flex-1 items-center gap-1.5"
        >
          {expanded ? (
            <ChevronDownIcon className="text-muted-foreground size-4 shrink-0" />
          ) : (
            <ChevronRightIcon className="text-muted-foreground size-4 shrink-0" />
          )}
          <ListChecksIcon className="text-muted-foreground size-4 shrink-0" />
          <span className="truncate text-sm font-medium">
            <Trans>Action items</Trans>
          </span>
          {count > 0 ? (
            <span className="text-muted-foreground text-xs">{count}</span>
          ) : null}
        </button>
        <Button
          size="sm"
          variant="outline"
          onClick={() => void handleExtract()}
          disabled={isExtracting}
          title={resolutionMessage ?? undefined}
        >
          {isExtracting ? (
            <Spinner size={14} />
          ) : (
            <SparklesIcon className="size-3.5" />
          )}
          <span>
            <Trans>Extract action items</Trans>
          </span>
        </Button>
      </header>

      {expanded ? (
        <div className="border-border/60 max-h-64 overflow-y-auto border-t px-1.5 py-1.5">
          {extractError ? (
            <p className="px-2 py-1.5 text-xs text-red-500">{extractError}</p>
          ) : null}

          {isLoading ? (
            <div className="text-muted-foreground flex items-center gap-2 px-2 py-3 text-xs">
              <Spinner size={14} />
              <Trans>Loading action items…</Trans>
            </div>
          ) : count === 0 ? (
            <p className="text-muted-foreground px-2 py-3 text-xs">
              <Trans>No action items yet</Trans>
            </p>
          ) : (
            <ul className="flex flex-col gap-0.5">
              {items.map((item) => (
                <ActionItemRow
                  key={item.id}
                  item={item}
                  sessionId={sessionId}
                  ownerLabel={ownerLabels.get(item.ownerSpeakerId)}
                  onToggle={handleToggle}
                />
              ))}
            </ul>
          )}
        </div>
      ) : null}
    </section>
  );
}

function ActionItemRow({
  item,
  sessionId,
  ownerLabel,
  onToggle,
}: {
  item: SessionActionItemRecord;
  sessionId: string;
  ownerLabel: string | undefined;
  onToggle: (item: SessionActionItemRecord) => void;
}) {
  const { t } = useLingui();
  const openNew = useTabs((state) => state.openNew);
  const isDone = item.status === "done";

  const handleJumpToSource = useCallback(() => {
    // Opening the session is the reachable half of jump-to-source; the actual
    // audio seek lives on a separate channel (same deferral the search page
    // took). TODO seek to item.sourceStartMs once the seek bridge is wired.
    openNew({ type: "sessions", id: sessionId });
  }, [openNew, sessionId]);

  return (
    <li className="hover:bg-accent/40 group flex items-start gap-2 rounded-md px-2 py-1.5">
      <Checkbox
        checked={isDone}
        onCheckedChange={() => onToggle(item)}
        aria-label={t`Toggle action item`}
        className="mt-0.5"
      />
      <div className="flex min-w-0 flex-1 flex-col gap-1">
        <div className="flex min-w-0 items-start gap-1.5">
          <ConfidenceDot confidence={item.confidence} />
          <span
            className={cn([
              "min-w-0 flex-1 text-sm",
              isDone && "text-muted-foreground line-through",
            ])}
          >
            {item.text}
          </span>
        </div>
        <div className="flex flex-wrap items-center gap-1.5">
          <PriorityBadge priority={item.priority} />
          {ownerLabel ? (
            <Badge variant="secondary" size="sm" className="gap-1">
              <UserIcon className="size-3" />
              {ownerLabel}
            </Badge>
          ) : null}
          {item.dueAt ? (
            <Badge variant="outline" size="sm" className="gap-1">
              <ClockIcon className="size-3" />
              {item.dueAt}
            </Badge>
          ) : null}
          {item.sourceStartMs != null ? (
            <button
              type="button"
              onClick={handleJumpToSource}
              title={t`Jump to source`}
              className="text-muted-foreground hover:text-foreground inline-flex items-center gap-1 rounded px-1 text-xs"
            >
              <CornerDownRightIcon className="size-3" />
              <Trans>Source</Trans>
            </button>
          ) : null}
        </div>
      </div>
    </li>
  );
}

function ConfidenceDot({ confidence }: { confidence: number }) {
  const { t } = useLingui();
  const level =
    confidence > 0.8 ? "high" : confidence >= 0.5 ? "medium" : "low";
  const color =
    level === "high"
      ? "bg-green-500"
      : level === "medium"
        ? "bg-amber-500"
        : "bg-red-400";
  const label =
    level === "high"
      ? t`High confidence`
      : level === "medium"
        ? t`Medium confidence`
        : t`Low confidence`;

  return (
    <span
      className={cn(["mt-1.5 size-2 shrink-0 rounded-full", color])}
      role="img"
      aria-label={label}
      title={label}
    />
  );
}

function PriorityBadge({ priority }: { priority: string }) {
  const { t } = useLingui();
  if (priority !== "low" && priority !== "medium" && priority !== "high") {
    return null;
  }

  const variant =
    priority === "high"
      ? "destructive"
      : priority === "medium"
        ? "default"
        : "secondary";
  const label =
    priority === "high" ? t`High` : priority === "medium" ? t`Medium` : t`Low`;

  return (
    <Badge variant={variant} size="sm">
      {label}
    </Badge>
  );
}

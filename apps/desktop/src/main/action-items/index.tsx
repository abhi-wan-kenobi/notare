import { Trans, useLingui } from "@lingui/react/macro";
import { cn } from "@hypr/utils";
import { format, isValid, parseISO } from "date-fns";
import { RepeatIcon } from "lucide-react";
import { useMemo, useState } from "react";

import {
  type AllActionItem,
  type AllActionItemFilter,
  useAllActionItems,
  useAllOpenActionItemsCount,
} from "./queries";

import { setActionItemStatus } from "~/session/components/action-items/queries";
import { StandardContentWrapper } from "~/shared/main";
import { useOwnerUserId } from "~/shared/owner-user";
import { useTabs } from "~/store/zustand/tabs";

const FILTERS: AllActionItemFilter[] = ["open", "mine", "overdue", "done"];

export function TabContentActionItems() {
  return (
    <StandardContentWrapper noBorder>
      <ActionItemsView />
    </StandardContentWrapper>
  );
}

function ActionItemsView() {
  const { t } = useLingui();
  const [filter, setFilter] = useState<AllActionItemFilter>("open");
  const ownerUserId = useOwnerUserId();

  const { items } = useAllActionItems(filter);
  const { items: doneItems } = useAllActionItems("done");
  const openCount = useAllOpenActionItemsCount();
  const openCurrent = useTabs((state) => state.openCurrent);

  const groups = useMemo(() => {
    const result = new Map<string, AllActionItem[]>();
    for (const item of items) {
      const group = result.get(item.session_id) ?? [];
      group.push(item);
      result.set(item.session_id, group);
    }
    return [...result.entries()];
  }, [items]);

  const toggle = (item: AllActionItem) => {
    if (!ownerUserId) return;
    void setActionItemStatus(
      item.id,
      item.status === "done" ? "todo" : "done",
      ownerUserId,
    ).catch((error: unknown) => {
      console.error("Failed to update action item status", error);
    });
  };

  const renderActionRow = (item: AllActionItem, sessionTitle: string) => {
    const isDone = item.status === "done";
    const dueDate = item.due_at ? parseISO(item.due_at) : null;
    const dueDay = item.due_at ? item.due_at.slice(0, 10) : "";
    const todayStr = format(new Date(), "yyyy-MM-dd");
    const isOverdue =
      !isDone &&
      /^\d{4}-\d{2}-\d{2}$/.test(dueDay) &&
      dueDay < todayStr;

    const isCarriedOver = Boolean(
      item.carried_from_session_id &&
        item.carried_from_session_id !== item.session_id,
    );

    const sourceDateRaw =
      isCarriedOver && item.carried_from_session_date
        ? item.carried_from_session_date
        : item.session_date;
    const sourceDate = sourceDateRaw ? parseISO(sourceDateRaw) : null;
    const sourceTitle =
      isCarriedOver && item.carried_from_session_title
        ? item.carried_from_session_title
        : sessionTitle || item.session_title;
    const sourceDateFormatted =
      sourceDate && isValid(sourceDate) ? format(sourceDate, "MMM d") : "";

    return (
      <div
        key={item.id}
        className={cn(
          "grid grid-cols-[18px_minmax(0,1fr)_118px_88px] max-sm:grid-cols-[18px_minmax(0,1fr)_74px] gap-3 items-start py-2.5 border-b border-border/40 text-xs",
          isDone && "text-muted-foreground/70",
        )}
      >
        <input
          type="checkbox"
          checked={isDone}
          onChange={() => toggle(item)}
          aria-label={item.text}
          className="accent-primary mt-1 size-3.5 rounded border-border cursor-pointer shrink-0"
        />
        <div className="leading-snug min-w-0">
          <span
            className={cn(
              "text-xs break-words",
              isDone && "line-through text-muted-foreground",
            )}
          >
            {item.text}
          </span>
          {isCarriedOver ? (
            <span className="inline-flex items-center ml-1.5 px-1.5 py-0.5 border border-border rounded-full text-[9px] text-muted-foreground align-baseline">
              <Trans>Carried over</Trans>
            </span>
          ) : null}
          {sourceDateFormatted && sourceTitle ? (
            <span className="block text-[10px] text-muted-foreground/80 mt-1">
              <Trans>
                from {sourceDateFormatted} · {sourceTitle}
              </Trans>
            </span>
          ) : null}
        </div>
        <span className="text-[11px] text-muted-foreground truncate max-sm:hidden">
          {item.owner_name}
        </span>
        <div className="text-right">
          {isDone ? (
            <span className="text-[11px] text-muted-foreground whitespace-nowrap">
              <Trans>Done</Trans>
            </span>
          ) : item.due_at ? (
            <span
              className={cn(
                "text-[11px] whitespace-nowrap",
                isOverdue
                  ? "text-[#9A5D57] dark:text-[#E07A70] font-medium"
                  : "text-muted-foreground",
              )}
            >
              <Trans>
                Due {dueDate && isValid(dueDate) ? format(dueDate, "MMM d") : item.due_at}
              </Trans>
            </span>
          ) : null}
        </div>
      </div>
    );
  };

  return (
    <div className="mx-auto w-full max-w-[800px] px-8 py-9">
      <div className="flex items-baseline gap-2.5">
        <h1 className="page-title">
          <Trans>Action items</Trans>
        </h1>
        <span className="text-muted-foreground text-xs">
          <Trans>{openCount} open</Trans>
        </span>
      </div>

      <div
        className="my-6 flex flex-wrap gap-2"
        role="group"
        aria-label={t`Filter action items`}
      >
        {FILTERS.map((value) => (
          <button
            type="button"
            key={value}
            aria-pressed={filter === value}
            onClick={() => setFilter(value)}
            className={cn(
              "h-[30px] px-3 rounded-full border text-xs transition-colors capitalize",
              filter === value
                ? "border-border bg-card text-foreground font-medium shadow-xs"
                : "border-transparent text-muted-foreground hover:text-foreground hover:bg-accent/40",
            )}
          >
            {value === "open" ? (
              <Trans>Open</Trans>
            ) : value === "mine" ? (
              <Trans>Mine</Trans>
            ) : value === "overdue" ? (
              <Trans>Overdue</Trans>
            ) : (
              <Trans>Done</Trans>
            )}
          </button>
        ))}
      </div>

      {groups.length === 0 ? (
        <p className="text-muted-foreground mt-6 text-sm">
          {filter === "done" ? (
            <Trans>No completed action items.</Trans>
          ) : (
            <Trans>No action items.</Trans>
          )}
        </p>
      ) : (
        groups.map(([sessionId, group]) => {
          const first = group[0]!;
          const sessionDate = first.session_date
            ? parseISO(first.session_date)
            : null;
          return (
            <section key={sessionId} className="mt-6">
              <h2 className="border-border flex items-baseline justify-between border-b pb-2 font-serif text-[15px] font-medium">
                <button
                  type="button"
                  onClick={() =>
                    openCurrent({ type: "sessions", id: sessionId })
                  }
                  className="hover:underline flex items-center gap-1.5 text-left text-foreground"
                >
                  <span>{first.session_title || <Trans>Untitled</Trans>}</span>
                  {first.series_id ? (
                    <RepeatIcon
                      size={13}
                      className="text-muted-foreground inline-block shrink-0"
                      aria-label={t`Recurring`}
                    />
                  ) : null}
                </button>
                <span className="text-muted-foreground font-sans text-[11px] shrink-0 ml-3">
                  {sessionDate && isValid(sessionDate)
                    ? format(sessionDate, "MMM d")
                    : ""}
                </span>
              </h2>
              <div>
                {group.map((item) =>
                  renderActionRow(item, first.session_title),
                )}
              </div>
            </section>
          );
        })
      )}

      {filter !== "done" && doneItems.length > 0 ? (
        <details className="done-disclosure mt-7 border-t border-border pt-3">
          <summary className="cursor-pointer text-xs text-muted-foreground font-medium hover:text-foreground">
            <Trans>Done ({doneItems.length})</Trans>
          </summary>
          <div className="mt-2">
            {doneItems.map((item) =>
              renderActionRow(item, item.session_title),
            )}
          </div>
        </details>
      ) : null}
    </div>
  );
}

import { Trans } from "@lingui/react/macro";
import { useCallback, useMemo } from "react";

import { useLiveQuery } from "~/db";
import { StandardContentWrapper } from "~/shared/main";
import { useTabs } from "~/store/zustand/tabs";

type OpenActionItemRow = {
  id: string;
  text: string;
  session_id: string;
  session_title: string | null;
};

type SessionGroup = {
  sessionId: string;
  sessionTitle: string | null;
  items: OpenActionItemRow[];
};

const EMPTY_ITEMS: OpenActionItemRow[] = [];

/**
 * Wave 1 placeholder: every open action item, grouped by its session.
 * Wave 2 (slice E) owns the real filters/mine/overdue/done view from
 * action-items.html.
 */
export function TabContentActionItems() {
  return (
    <StandardContentWrapper>
      <ActionItemsView />
    </StandardContentWrapper>
  );
}

function ActionItemsView() {
  const { data: items = EMPTY_ITEMS } = useLiveQuery<
    OpenActionItemRow,
    OpenActionItemRow[]
  >({
    sql: `
      SELECT
        action_items.id,
        action_items.text,
        action_items.session_id,
        sessions.title AS session_title
      FROM action_items
      JOIN sessions ON sessions.id = action_items.session_id
        AND sessions.deleted_at IS NULL
      WHERE action_items.deleted_at IS NULL
        AND action_items.status != 'done'
      ORDER BY sessions.created_at DESC, action_items.source_order, action_items.created_at
    `,
  });

  const openCurrent = useTabs((state) => state.openCurrent);
  const openSession = useCallback(
    (sessionId: string) => {
      openCurrent({ type: "sessions", id: sessionId });
    },
    [openCurrent],
  );

  const groups = useMemo<SessionGroup[]>(() => {
    const bySession = new Map<string, SessionGroup>();
    for (const item of items) {
      const existing = bySession.get(item.session_id);
      if (existing) {
        existing.items.push(item);
      } else {
        bySession.set(item.session_id, {
          sessionId: item.session_id,
          sessionTitle: item.session_title,
          items: [item],
        });
      }
    }
    return Array.from(bySession.values());
  }, [items]);

  return (
    <div className="mx-auto w-full max-w-[700px] px-8 py-9">
      <div className="flex items-baseline gap-2">
        <h1 className="page-title">
          <Trans>Action items</Trans>
        </h1>
        <span className="text-muted-foreground text-xs">
          <Trans>{items.length} open</Trans>
        </span>
      </div>

      {groups.length === 0 ? (
        <p className="text-muted-foreground mt-6 text-sm">
          <Trans>No open action items.</Trans>
        </p>
      ) : (
        groups.map((group) => (
          <section key={group.sessionId} className="mt-7">
            <button
              type="button"
              onClick={() => openSession(group.sessionId)}
              className="hover:text-foreground text-muted-foreground border-border w-full border-b pb-1.5 text-left font-serif text-[15px] font-medium"
            >
              {group.sessionTitle || <Trans>Untitled</Trans>}
            </button>
            <div className="divide-border mt-1 divide-y">
              {group.items.map((item) => (
                <div key={item.id} className="flex items-start gap-3 py-2.5">
                  <input
                    type="checkbox"
                    className="mt-0.5 size-3.5"
                    aria-label={item.text}
                    readOnly
                  />
                  <span className="flex-1 text-sm">{item.text}</span>
                </div>
              ))}
            </div>
          </section>
        ))
      )}
    </div>
  );
}

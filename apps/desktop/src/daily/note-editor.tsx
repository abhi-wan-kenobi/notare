import { useCallback, useRef } from "react";

import { parseJsonContent } from "@hypr/tiptap/shared";

import { useCalendarData } from "~/calendar/hooks";
import { type JSONContent, NoteEditor } from "~/editor/session";
import {
  getNodeTextContent,
  mergeLinkedSessionsIntoContent,
} from "~/editor/session/linked-session-content";
import { findSessionByEventId } from "~/session/utils";
import * as main from "~/store/tinybase/store/main";
import { getOrCreateSessionForEventId } from "~/store/tinybase/store/sessions";

type Store = NonNullable<ReturnType<typeof main.UI.useStore>>;

function getSessionTitle(store: Store, sessionId: string): string {
  const title = store.getCell("sessions", sessionId, "title");
  return typeof title === "string" ? title : "";
}

function resolveEventSessionId(store: Store, eventId: string): string | null {
  const existingSessionId = findSessionByEventId(store, eventId);
  if (existingSessionId) {
    return existingSessionId;
  }

  const event = store.getRow("events", eventId);
  if (!event) {
    return null;
  }

  return getOrCreateSessionForEventId(store, eventId, event.title as string);
}

function buildInitialContent(
  store: Store,
  content: unknown,
  eventIds: string[],
  sessionIds: string[],
): JSONContent {
  return mergeLinkedSessionsIntoContent({
    content: parseJsonContent(content as string),
    eventIds,
    sessionIds,
    resolveEventSessionId: (eventId) => resolveEventSessionId(store, eventId),
    getSessionTitle: (sessionId) => getSessionTitle(store, sessionId),
  });
}

export function DailyNoteEditor({ date }: { date: string }) {
  const store = main.UI.useStore(main.STORE_ID);
  const content = main.UI.useCell(
    "daily_notes",
    date,
    "content",
    main.STORE_ID,
  );

  const { eventIdsByDate, sessionIdsByDate } = useCalendarData();
  const initialContentRef = useRef<JSONContent | null>(null);
  const initialContentDateRef = useRef<string | null>(null);

  if (
    (initialContentDateRef.current !== date || !initialContentRef.current) &&
    store
  ) {
    initialContentRef.current = buildInitialContent(
      store,
      content,
      eventIdsByDate[date] ?? [],
      sessionIdsByDate[date] ?? [],
    );
    initialContentDateRef.current = date;
  }

  const persistDailyNote = main.UI.useSetPartialRowCallback(
    "daily_notes",
    date,
    (input: JSONContent) => ({ content: JSON.stringify(input), date }),
    [date],
    main.STORE_ID,
  );

  const handleChange = useCallback(
    (input: JSONContent) => {
      if (store) {
        for (const node of input.content ?? []) {
          if (node.type !== "session") {
            continue;
          }

          const sessionId = node.attrs?.sessionId;
          if (typeof sessionId !== "string" || sessionId === "") {
            continue;
          }

          const nextTitle = getNodeTextContent(node);
          const currentTitle = getSessionTitle(store, sessionId);
          if (nextTitle !== currentTitle) {
            store.setPartialRow("sessions", sessionId, { title: nextTitle });
          }
        }
      }

      persistDailyNote(input);
    },
    [persistDailyNote, store],
  );

  if (!initialContentRef.current) {
    return null;
  }

  return (
    <div className="px-2">
      <NoteEditor
        key={`daily-${date}`}
        initialContent={initialContentRef.current}
        handleChange={handleChange}
        linkedItemOpenBehavior="new"
      />
    </div>
  );
}

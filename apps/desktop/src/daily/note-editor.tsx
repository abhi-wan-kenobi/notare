import { useCallback, useRef } from "react";

import { parseJsonContent } from "@hypr/tiptap/shared";

import { useCalendarData } from "~/calendar/hooks";
import { type JSONContent, NoteEditor } from "~/editor/session";
import { findSessionByEventId } from "~/session/utils";
import * as main from "~/store/tinybase/store/main";
import { getOrCreateSessionForEventId } from "~/store/tinybase/store/sessions";

type Store = NonNullable<ReturnType<typeof main.UI.useStore>>;

function getNodeTextContent(node: JSONContent): string {
  if (typeof node.text === "string") {
    return node.text;
  }

  return (node.content ?? []).map(getNodeTextContent).join("");
}

function buildTextContent(text: string): JSONContent[] | undefined {
  return text ? [{ type: "text", text }] : undefined;
}

function getSessionTitle(store: Store, sessionId: string): string {
  const title = store.getCell("sessions", sessionId, "title");
  return typeof title === "string" ? title : "";
}

function buildSessionNode(sessionId: string, title: string): JSONContent {
  return {
    type: "session",
    attrs: { sessionId },
    content: buildTextContent(title),
  };
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
  const parsed = parseJsonContent(content as string);
  const existingContent = parsed.content ?? [];
  const seenSessionIds = new Set<string>();
  const linkedSessionNodes: JSONContent[] = [];

  const pushSessionNode = (sessionId: string, preferredTitle?: string) => {
    if (!sessionId || seenSessionIds.has(sessionId)) {
      return;
    }

    seenSessionIds.add(sessionId);
    linkedSessionNodes.push(
      buildSessionNode(
        sessionId,
        preferredTitle ?? getSessionTitle(store, sessionId),
      ),
    );
  };

  for (const node of existingContent) {
    if (node.type === "session") {
      const sessionId = node.attrs?.sessionId;
      if (typeof sessionId !== "string" || sessionId === "") {
        continue;
      }

      pushSessionNode(
        sessionId,
        getNodeTextContent(node) || getSessionTitle(store, sessionId),
      );
      continue;
    }

    if (node.type === "event") {
      const eventId = node.attrs?.eventId;
      if (typeof eventId !== "string" || eventId === "") {
        continue;
      }

      const sessionId = resolveEventSessionId(store, eventId);
      if (!sessionId) {
        continue;
      }

      pushSessionNode(
        sessionId,
        getNodeTextContent(node) || getSessionTitle(store, sessionId),
      );
    }
  }

  for (const eventId of eventIds) {
    const sessionId = resolveEventSessionId(store, eventId);
    if (sessionId) {
      pushSessionNode(sessionId);
    }
  }

  for (const sessionId of sessionIds) {
    pushSessionNode(sessionId);
  }

  const userContent = existingContent.filter(
    (node) => node.type !== "event" && node.type !== "session",
  );
  const merged = [...linkedSessionNodes, ...userContent];

  if (merged.length === 0) {
    merged.push({ type: "paragraph" });
  }

  return { type: "doc", content: merged };
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

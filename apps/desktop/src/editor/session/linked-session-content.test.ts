import { describe, expect, it } from "vitest";

import { mergeLinkedSessionsIntoContent } from "./linked-session-content";

describe("mergeLinkedSessionsIntoContent", () => {
  it("deduplicates existing session and event-backed content", () => {
    const result = mergeLinkedSessionsIntoContent({
      content: {
        type: "doc",
        content: [
          {
            type: "session",
            attrs: { sessionId: "session-1" },
            content: [{ type: "text", text: "Existing session title" }],
          },
          {
            type: "event",
            attrs: { eventId: "event-1" },
            content: [{ type: "text", text: "Legacy event title" }],
          },
          {
            type: "paragraph",
            content: [{ type: "text", text: "Body content" }],
          },
        ],
      },
      eventIds: ["event-1", "event-2"],
      sessionIds: ["session-1", "session-3"],
      resolveEventSessionId: (eventId) => {
        if (eventId === "event-1") {
          return "session-1";
        }
        if (eventId === "event-2") {
          return "session-2";
        }
        return null;
      },
      getSessionTitle: (sessionId) =>
        ({
          "session-1": "Session 1",
          "session-2": "Session 2",
          "session-3": "Session 3",
        })[sessionId] ?? "",
    });

    expect(result).toEqual({
      type: "doc",
      content: [
        {
          type: "session",
          attrs: { sessionId: "session-1" },
          content: [{ type: "text", text: "Existing session title" }],
        },
        {
          type: "session",
          attrs: { sessionId: "session-2" },
          content: [{ type: "text", text: "Session 2" }],
        },
        {
          type: "session",
          attrs: { sessionId: "session-3" },
          content: [{ type: "text", text: "Session 3" }],
        },
        {
          type: "paragraph",
          content: [{ type: "text", text: "Body content" }],
        },
      ],
    });
  });

  it("falls back to an empty paragraph when no linked or user content remains", () => {
    const result = mergeLinkedSessionsIntoContent({
      content: {
        type: "doc",
        content: [{ type: "event", attrs: { eventId: "missing" } }],
      },
      eventIds: [],
      sessionIds: [],
      resolveEventSessionId: () => null,
      getSessionTitle: () => "",
    });

    expect(result).toEqual({
      type: "doc",
      content: [{ type: "paragraph" }],
    });
  });
});

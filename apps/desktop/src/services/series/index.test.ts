import { renderHook } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import {
  computeDurationMinutes,
  extractTldr,
  extractTopics,
  mapOccurrenceRow,
  parseDecisionsFromNote,
  parseParticipants,
  useSeriesDelta,
} from "./index";

describe("useSeriesDelta", () => {
  it("reports topics added and removed between consecutive occurrences", () => {
    const { result } = renderHook(() =>
      useSeriesDelta(
        { enhanced_note: "## Decisions\n## Launch plan" } as never,
        { enhanced_note: "## Decisions\n## Risks" } as never,
      ),
    );

    expect(result.current).toEqual({
      decisions: [],
      actionsClosed: [],
      attendeesJoined: [],
      attendeesLeft: [],
      topicsAdded: ["Launch plan"],
      topicsRemoved: ["Risks"],
    });
  });

  it("extracts decisions from fenced note block and attendee diffs from event_json", () => {
    const current = {
      enhanced_note:
        '```decisions\n[{"text": "Ship v0.7 on Tuesday", "owner": "Alice"}]\n```\n## Roadmap',
      event_json: JSON.stringify({
        participants: [
          { name: "Alice", email: "alice@test.com" },
          { name: "Charlie", email: "charlie@test.com" },
        ],
      }),
    };

    const previous = {
      enhanced_note: "## Old Topic",
      event_json: JSON.stringify({
        participants: [
          { name: "Alice", email: "alice@test.com" },
          { name: "Bob", email: "bob@test.com" },
        ],
      }),
    };

    const { result } = renderHook(() =>
      useSeriesDelta(current as never, previous as never),
    );

    expect(result.current.decisions).toEqual([
      { text: "Ship v0.7 on Tuesday", owner: "Alice" },
    ]);
    expect(result.current.topicsAdded).toEqual(["Roadmap"]);
    expect(result.current.topicsRemoved).toEqual(["Old Topic"]);
    expect(result.current.attendeesJoined).toEqual([
      { name: "Charlie", email: "charlie@test.com" },
    ]);
    expect(result.current.attendeesLeft).toEqual([
      { name: "Bob", email: "bob@test.com" },
    ]);
  });
});

describe("extractTldr", () => {
  it("extracts first non-empty line and strips markdown markers", () => {
    expect(
      extractTldr("## Executive Summary\nLaunch target set for December 1."),
    ).toBe("Executive Summary");

    expect(
      extractTldr("- Key takeaway: release is ready\nSecond line details"),
    ).toBe("Key takeaway: release is ready");

    expect(extractTldr("```decisions\n[]\n```\nFirst real text")).toBe(
      "First real text",
    );

    expect(extractTldr("")).toBe("");
    expect(extractTldr(null)).toBe("");
  });
});
describe("extractTopics", () => {
  it("extracts all H2 and higher markdown headings", () => {
    const markdown =
      "# Title\n## Topic One\n### Subtopic\n## Topic Two\nRegular text";
    expect(extractTopics(markdown)).toEqual([
      "Topic One",
      "Subtopic",
      "Topic Two",
    ]);
    expect(extractTopics("")).toEqual([]);
    expect(extractTopics(null)).toEqual([]);
  });
});

describe("parseDecisionsFromNote", () => {
  it("parses decisions from JSON in fenced code blocks", () => {
    const note =
      '```decisions\n[{"text":"Approved budget","owner":"Dave"}]\n```';
    expect(parseDecisionsFromNote(note)).toEqual([
      { text: "Approved budget", owner: "Dave" },
    ]);
    expect(parseDecisionsFromNote("no decisions here")).toEqual([]);
  });
});

describe("parseParticipants", () => {
  it("extracts participants from event_json", () => {
    const eventJson = JSON.stringify({
      participants: [
        { name: "John Doe", email: "john@example.com" },
        { display_name: "Jane Doe" },
      ],
    });
    expect(parseParticipants(eventJson)).toEqual([
      { name: "John Doe", email: "john@example.com" },
      { name: "Jane Doe", email: undefined },
    ]);
    expect(parseParticipants("invalid json")).toEqual([]);
    expect(parseParticipants(null)).toEqual([]);
  });
});

describe("computeDurationMinutes", () => {
  it("calculates positive duration in minutes", () => {
    expect(
      computeDurationMinutes(
        "2026-11-05T10:00:00.000Z",
        "2026-11-05T10:45:00.000Z",
      ),
    ).toBe(45);

    expect(
      computeDurationMinutes(
        "2026-11-05T10:00:00.000Z",
        "2026-11-05T10:00:00.000Z",
      ),
    ).toBe(0);

    expect(computeDurationMinutes("", "2026-11-05T10:45:00.000Z")).toBe(0);
    expect(computeDurationMinutes("invalid", "invalid")).toBe(0);
  });
});

describe("mapOccurrenceRow", () => {
  it("maps SQL row into SeriesOccurrence with calculated fields", () => {
    const row = {
      id: "sess-1",
      title: "Weekly Sync",
      started_at: "2026-11-05T14:00:00.000Z",
      ended_at: "2026-11-05T14:30:00.000Z",
      created_at: "2026-11-05T13:50:00.000Z",
      event_json: "{}",
      enhanced_note: "## Summary\nAll deliverables on track.",
      open_item_count: 3,
    };

    const occurrence = mapOccurrenceRow(row);
    expect(occurrence.id).toBe("sess-1");
    expect(occurrence.title).toBe("Weekly Sync");
    expect(occurrence.date).toBe("2026-11-05T14:00:00.000Z");
    expect(occurrence.duration_minutes).toBe(30);
    expect(occurrence.tldr).toBe("Summary");
    expect(occurrence.open_item_count).toBe(3);
  });
});

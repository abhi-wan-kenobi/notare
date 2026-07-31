import { addHours, startOfDay, subDays } from "@hypr/utils";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { groupEntriesByDay } from "./day-grouping";

import type { DictationHistoryEntry } from "~/dictation/history";

function makeEntry(
  overrides: Partial<DictationHistoryEntry> & { id: string; createdAt: string },
): DictationHistoryEntry {
  return {
    text: "hello",
    rawText: null,
    source: "dictation",
    model: null,
    durationMs: null,
    pinned: false,
    status: "delivered",
    ...overrides,
  };
}

// Pin "now" and derive every fixture timestamp from it via date-fns so the
// test is immune to the runner's timezone (day-grouping.ts uses local-time
// `startOfDay`/`isToday`/`isYesterday`, so hardcoded UTC-midnight-adjacent
// ISO strings would be flaky across timezones).
const NOW = new Date("2026-07-31T12:00:00.000Z");

function localTimeOn(daysAgo: number, hour = 9): string {
  return addHours(startOfDay(subDays(NOW, daysAgo)), hour).toISOString();
}

describe("groupEntriesByDay", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.setSystemTime(NOW);
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("returns no buckets for an empty list", () => {
    expect(groupEntriesByDay([])).toEqual([]);
  });

  it("groups same-day entries into a single 'today' bucket, preserving order", () => {
    const entries = [
      makeEntry({ id: "1", createdAt: localTimeOn(0, 11) }),
      makeEntry({ id: "2", createdAt: localTimeOn(0, 8) }),
    ];

    const buckets = groupEntriesByDay(entries);

    expect(buckets).toHaveLength(1);
    expect(buckets[0].bucket).toEqual({ kind: "today" });
    expect(buckets[0].entries.map((e) => e.id)).toEqual(["1", "2"]);
  });

  it("labels today, yesterday and older dates distinctly", () => {
    const entries = [
      makeEntry({ id: "today", createdAt: localTimeOn(0) }),
      makeEntry({ id: "yesterday", createdAt: localTimeOn(1) }),
      makeEntry({ id: "older", createdAt: localTimeOn(10) }),
    ];

    const buckets = groupEntriesByDay(entries);

    expect(buckets.map((b) => b.bucket.kind)).toEqual([
      "today",
      "yesterday",
      "date",
    ]);
    const olderBucket = buckets[2].bucket;
    expect(olderBucket.kind === "date" && olderBucket.date.getTime()).toBe(
      startOfDay(subDays(NOW, 10)).getTime(),
    );
  });

  it("does not merge non-adjacent same-day runs (no re-sorting)", () => {
    const entries = [
      makeEntry({ id: "a", createdAt: localTimeOn(0, 11) }),
      makeEntry({ id: "b", createdAt: localTimeOn(1, 9) }),
      makeEntry({ id: "c", createdAt: localTimeOn(0, 8) }),
    ];

    const buckets = groupEntriesByDay(entries);

    expect(buckets).toHaveLength(3);
    expect(buckets.map((b) => b.entries[0]?.id)).toEqual(["a", "b", "c"]);
  });

  it("keeps entries with an unparseable createdAt visible in an 'unknown' bucket", () => {
    const entries = [
      makeEntry({ id: "bad", createdAt: "not-a-date" }),
      makeEntry({ id: "also-bad", createdAt: "" }),
    ];

    const buckets = groupEntriesByDay(entries);

    expect(buckets).toHaveLength(1);
    expect(buckets[0].key).toBe("unknown");
    expect(buckets[0].bucket).toEqual({ kind: "unknown" });
    expect(buckets[0].entries.map((e) => e.id)).toEqual(["bad", "also-bad"]);
  });
});

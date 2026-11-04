import { renderHook } from "@testing-library/react";
import type { DatabaseSync } from "node:sqlite";
import { beforeEach, describe, expect, test, vi } from "vitest";

import {
  createMigratedDb,
  execAll,
  insertCalendar,
  insertEvent,
  queryAll,
} from "./test-sqlite";

const state = vi.hoisted(() => ({ db: null as unknown as DatabaseSync }));

vi.mock("~/db", () => ({
  executeTransaction: async (
    statements: Array<{ sql: string; params?: unknown[] }>,
  ) => execAll(state.db, statements),
  liveQueryClient: {
    execute: async (sql: string, params?: unknown[]) =>
      queryAll(state.db, sql, params),
  },
  useLiveQuery: (options: {
    sql: string;
    params?: unknown[];
    mapRows: (rows: unknown[]) => unknown;
  }) => ({
    data: options.mapRows(queryAll(state.db, options.sql, options.params)),
  }),
}));

vi.mock("~/db/write-queue", () => ({
  enqueueDatabaseWrite: (_key: string, write: () => Promise<unknown>) =>
    write(),
}));

import {
  getHiddenCalendarProviders,
  parseHiddenCalendarProviders,
  setCalendarProviderHidden,
} from "./hidden-providers";
import { useCalendarData } from "./hooks";
import { useTimelineEventsTable } from "./queries";

function seedProviders() {
  insertCalendar(state.db, { id: "cal-apple", provider: "apple" });
  insertCalendar(state.db, { id: "cal-google", provider: "google" });
  insertCalendar(state.db, { id: "cal-ics", provider: "ics", enabled: false });
  insertEvent(state.db, {
    id: "apple-event",
    calendarId: "cal-apple",
    startedAt: "2026-11-04T09:00:00.000Z",
  });
  insertEvent(state.db, {
    id: "google-event",
    calendarId: "cal-google",
    startedAt: "2026-11-04T10:00:00.000Z",
  });
  insertEvent(state.db, {
    id: "orphan-event",
    calendarId: "missing-calendar",
    startedAt: "2026-11-04T11:00:00.000Z",
  });
}

function calendarFlags() {
  return queryAll<{ id: string; enabled: number; deleted_at: string | null }>(
    state.db,
    `SELECT id, enabled, deleted_at FROM calendars ORDER BY id`,
  );
}

function liveEventCount() {
  return queryAll<{ n: number }>(
    state.db,
    `SELECT COUNT(*) AS n FROM events WHERE deleted_at IS NULL`,
  )[0].n;
}

describe("hidden calendar providers", () => {
  beforeEach(() => {
    state.db = createMigratedDb();
    seedProviders();
  });

  test("timeline events drop a hidden provider and return when it is shown", async () => {
    const { result, rerender } = renderHook(() => useTimelineEventsTable());
    expect(Object.keys(result.current ?? {}).sort()).toEqual([
      "apple-event",
      "google-event",
      "orphan-event",
    ]);

    await setCalendarProviderHidden("apple", true);
    rerender();
    expect(Object.keys(result.current ?? {}).sort()).toEqual([
      "google-event",
      "orphan-event",
    ]);

    await setCalendarProviderHidden("google", true);
    rerender();
    expect(Object.keys(result.current ?? {})).toEqual(["orphan-event"]);

    await setCalendarProviderHidden("apple", false);
    await setCalendarProviderHidden("google", false);
    rerender();
    expect(Object.keys(result.current ?? {}).sort()).toEqual([
      "apple-event",
      "google-event",
      "orphan-event",
    ]);
  });

  test("calendar data omits hidden provider events by day and restores them", async () => {
    const { result, rerender } = renderHook(() => useCalendarData());
    const day = Object.keys(result.current.eventIdsByDate)[0];
    expect([...result.current.eventIdsByDate[day]].sort()).toEqual([
      "apple-event",
      "google-event",
      "orphan-event",
    ]);

    await setCalendarProviderHidden("google", true);
    rerender();
    expect(Object.keys(result.current.eventsById).sort()).toEqual([
      "apple-event",
      "orphan-event",
    ]);
    expect([...result.current.eventIdsByDate[day]].sort()).toEqual([
      "apple-event",
      "orphan-event",
    ]);

    await setCalendarProviderHidden("google", false);
    rerender();
    expect(result.current.eventIdsByDate[day]).toContain("google-event");
  });

  test("hiding never changes calendar enabled flags or tombstones events", async () => {
    const before = calendarFlags();
    expect(liveEventCount()).toBe(3);

    await setCalendarProviderHidden("apple", true);
    await setCalendarProviderHidden("ics", true);

    expect(calendarFlags()).toEqual(before);
    expect(before.map((calendar) => calendar.enabled)).toEqual([1, 1, 0]);
    expect(liveEventCount()).toBe(3);
  });

  test("hidden provider ids persist deduplicated and sorted", async () => {
    await setCalendarProviderHidden("outlook", true);
    await setCalendarProviderHidden("apple", true);
    await setCalendarProviderHidden("apple", true);
    expect(await getHiddenCalendarProviders()).toEqual(["apple", "outlook"]);

    await setCalendarProviderHidden("outlook", false);
    expect(await getHiddenCalendarProviders()).toEqual(["apple"]);
  });

  test("a deleted calendar's events are not treated as hidden", async () => {
    execAll(state.db, [
      {
        sql: `UPDATE calendars SET deleted_at = '2026-11-02T00:00:00.000Z' WHERE id = 'cal-apple'`,
      },
    ]);
    await setCalendarProviderHidden("apple", true);

    const { result } = renderHook(() => useTimelineEventsTable());
    expect(Object.keys(result.current ?? {})).toContain("apple-event");
  });
});

describe("parseHiddenCalendarProviders", () => {
  test("accepts JSON arrays and ignores malformed or non-string entries", () => {
    expect(
      parseHiddenCalendarProviders('["google","apple","google",3]'),
    ).toEqual(["apple", "google"]);
    expect(parseHiddenCalendarProviders("not json")).toEqual([]);
    expect(parseHiddenCalendarProviders('{"a":1}')).toEqual([]);
    expect(parseHiddenCalendarProviders(undefined)).toEqual([]);
  });
});

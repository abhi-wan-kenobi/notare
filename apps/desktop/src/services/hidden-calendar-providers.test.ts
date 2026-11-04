import type { DatabaseSync } from "node:sqlite";
import { beforeEach, describe, expect, test, vi } from "vitest";

import {
  createMigratedDb,
  execAll,
  insertCalendar,
  insertEvent,
  queryAll,
} from "~/calendar/test-sqlite";

const state = vi.hoisted(() => ({ db: null as unknown as DatabaseSync }));
const notifications = vi.hoisted(() => ({ showNotification: vi.fn() }));

vi.mock("~/db", () => ({
  executeTransaction: async (
    statements: Array<{ sql: string; params?: unknown[] }>,
  ) => execAll(state.db, statements),
  liveQueryClient: {
    execute: async (sql: string, params?: unknown[]) =>
      queryAll(state.db, sql, params),
  },
}));

vi.mock("~/db/write-queue", () => ({
  enqueueDatabaseWrite: (_key: string, write: () => Promise<unknown>) =>
    write(),
}));

vi.mock("@hypr/plugin-notification", () => ({
  commands: { showNotification: notifications.showNotification },
}));

vi.mock("~/calendar/ignored-events", () => ({
  getIgnoredEventSets: async () => ({
    ignoredIds: new Set(),
    ignoredSeriesIds: new Set(),
  }),
}));

vi.mock("~/services/briefs/generator", () => ({
  generateBriefForEvent: vi.fn(),
}));

vi.mock("~/services/briefs/storage", () => ({
  loadBriefedEventIds: async () => new Set(),
}));

vi.mock("~/services/llm-router", () => ({ useTaskModel: vi.fn() }));

import { setCalendarProviderHidden } from "~/calendar/hidden-providers";
import { fetchCalendarEvents } from "~/services/briefs/scheduler";
import { checkEventNotifications } from "~/services/event-notification";

const NOW = new Date("2026-11-04T12:00:00.000Z");

describe("hidden calendar providers in services", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.spyOn(Date, "now").mockReturnValue(NOW.getTime());
    state.db = createMigratedDb();
    insertCalendar(state.db, { id: "cal-apple", provider: "apple" });
    insertCalendar(state.db, { id: "cal-google", provider: "google" });
    for (const [id, calendarId] of [
      ["apple-event", "cal-apple"],
      ["google-event", "cal-google"],
    ]) {
      insertEvent(state.db, {
        id,
        calendarId,
        startedAt: new Date(NOW.getTime() + 3 * 60_000).toISOString(),
      });
    }
  });

  test("pre-meeting brief candidates skip hidden providers and return on show", async () => {
    const ids = async () =>
      (await fetchCalendarEvents()).map((event) => event.id).sort();

    expect(await ids()).toEqual(["apple-event", "google-event"]);

    await setCalendarProviderHidden("apple", true);
    expect(await ids()).toEqual(["google-event"]);

    await setCalendarProviderHidden("apple", false);
    expect(await ids()).toEqual(["apple-event", "google-event"]);
  });

  test("starting-soon notifications skip hidden providers", async () => {
    await setCalendarProviderHidden("apple", true);

    await checkEventNotifications(true, new Map());

    expect(notifications.showNotification).toHaveBeenCalledTimes(1);
    expect(notifications.showNotification).toHaveBeenCalledWith(
      expect.objectContaining({
        source: { type: "calendar_event", event_id: "google-event" },
      }),
    );
  });
});

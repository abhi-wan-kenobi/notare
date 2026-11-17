import { cleanup, render, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { MenuItemDef } from "~/shared/hooks/useNativeContextMenu";
import type { TimelineEventRow } from "~/sidebar/timeline/utils";

const mocks = vi.hoisted(() => ({
  ask: vi.fn(),
  ignoreEvent: vi.fn(),
  ignoreSeries: vi.fn(),
  menu: [] as unknown[],
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({ ask: mocks.ask }));
vi.mock("~/calendar/hooks", () => ({
  toTz: (date: Date | string) => new Date(date),
  useTimezone: () => undefined,
}));
vi.mock("~/calendar/ignored-events", () => ({
  useIgnoredEvents: () => ({
    ignoreEvent: mocks.ignoreEvent,
    ignoreSeries: mocks.ignoreSeries,
  }),
}));
vi.mock("~/services/briefs", () => ({ BriefCard: () => null }));
vi.mock("~/session/components/outer-header/metadata", () => ({
  EventDisplay: () => null,
}));
vi.mock("~/session/queries", () => ({
  getOrCreateSessionForEventId: vi.fn(),
}));
vi.mock("~/shared/hooks/useNativeContextMenu", () => ({
  useNativeContextMenu: (items: unknown[]) => {
    mocks.menu = items;
    return vi.fn();
  },
}));
vi.mock("~/store/zustand/tabs", () => ({
  useTabs: () => vi.fn(),
}));

import { EventChip } from "./event-chip";

type MenuAction = Extract<MenuItemDef, { id: string }>;

const renderChip = (event: Partial<TimelineEventRow>) => {
  render(
    <EventChip
      eventId="event-1"
      event={
        {
          title: "Weekly sync",
          tracking_id_event: "tracking-1",
          started_at: "2026-11-05T15:30:00.000Z",
          ...event,
        } as TimelineEventRow
      }
    />,
  );
  return mocks.menu as MenuAction[];
};

describe("calendar event chip context menu", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.menu = [];
  });
  afterEach(cleanup);

  it("labels the single-event action as hiding, not deleting", () => {
    const menu = renderChip({ recurrence_series_id: null });

    expect(menu.map((item) => item.text)).toEqual(["Hide event"]);

    menu[0].action();
    expect(mocks.ignoreEvent).toHaveBeenCalledWith("tracking-1");
    expect(mocks.ask).not.toHaveBeenCalled();
  });

  it("hides a recurring series only after confirmation", async () => {
    mocks.ask.mockResolvedValue(true);
    const menu = renderChip({ recurrence_series_id: "series-1" });

    expect(menu.map((item) => item.text)).toEqual([
      "Hide this event",
      "Hide all recurring events",
    ]);

    menu[1].action();
    expect(mocks.ignoreSeries).not.toHaveBeenCalled();

    await waitFor(() =>
      expect(mocks.ignoreSeries).toHaveBeenCalledWith("series-1"),
    );
  });

  it("keeps a recurring series when the confirmation is cancelled", async () => {
    mocks.ask.mockResolvedValue(false);
    const menu = renderChip({ recurrence_series_id: "series-1" });

    menu[1].action();

    await waitFor(() => expect(mocks.ask).toHaveBeenCalledTimes(1));
    await Promise.resolve();
    expect(mocks.ignoreSeries).not.toHaveBeenCalled();
  });
});

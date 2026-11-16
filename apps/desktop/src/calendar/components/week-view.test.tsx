import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

// `now.getHours()` in the component reads the host's local time; pin UTC so
// the scroll-position assertions below don't depend on the test runner's
// system timezone (this machine runs Asia/Kolkata).
process.env.TZ = "UTC";

import type { MenuItemDef } from "~/shared/hooks/useNativeContextMenu";

const mocks = vi.hoisted(() => ({
  openCurrent: vi.fn(),
  getOrCreateSessionForEventId: vi.fn(async () => "session-week-1"),
  ignoreEvent: vi.fn(),
  ignoreSeries: vi.fn(),
  deleteSession: vi.fn(),
  showContextMenu: vi.fn<(items: MenuItemDef[]) => void>(),
  now: new Date("2026-11-04T10:00:00Z"),
}));

vi.mock("@lingui/react/macro", () => ({
  Trans: ({ children }: { children: ReactNode }) => <>{children}</>,
  useLingui: () => ({
    t: (input: TemplateStringsArray | string) =>
      typeof input === "string" ? input : input[0],
  }),
}));

vi.mock("@hypr/plugin-fs-sync", () => ({
  commands: { sessionDir: vi.fn() },
}));

vi.mock("@hypr/plugin-opener2", () => ({
  commands: { openPath: vi.fn() },
}));

vi.mock("~/session/hooks/useDeleteSession", () => ({
  useDeleteSession: () => mocks.deleteSession,
}));

vi.mock("~/store/zustand/tabs", () => ({
  useTabs: Object.assign(
    (selector: (state: unknown) => unknown) =>
      selector({
        openCurrent: mocks.openCurrent,
      }),
    {
      getState: () => ({
        openCurrent: mocks.openCurrent,
      }),
    },
  ),
}));

vi.mock("~/session/queries", () => ({
  getOrCreateSessionForEventId: mocks.getOrCreateSessionForEventId,
}));

vi.mock("~/calendar/ignored-events", () => ({
  useIgnoredEvents: () => ({
    ignoreEvent: mocks.ignoreEvent,
    ignoreSeries: mocks.ignoreSeries,
  }),
}));

vi.mock("~/shared/hooks/useNativeContextMenu", () => ({
  useNativeContextMenu: (items: MenuItemDef[]) => (event: MouseEvent) => {
    event.preventDefault();
    mocks.showContextMenu(items);
  },
}));

vi.mock("~/calendar/hooks", () => ({
  toTz: (iso: string) => new Date(iso),
  useNow: () => mocks.now,
  useTimezone: () => "UTC",
}));

import { WeekView } from "./week-view";

const mockDays = [
  new Date("2026-11-02T00:00:00Z"),
  new Date("2026-11-03T00:00:00Z"),
  new Date("2026-11-04T00:00:00Z"),
  new Date("2026-11-05T00:00:00Z"),
  new Date("2026-11-06T00:00:00Z"),
  new Date("2026-11-07T00:00:00Z"),
  new Date("2026-11-08T00:00:00Z"),
];

const mockCalendarData = {
  eventIdsByDate: {
    "2026-11-04": ["event-1", "event-2"],
    "2026-11-05": ["event-allday"],
  },
  eventsById: {
    "event-1": {
      id: "event-1",
      title: "Sprint Planning",
      started_at: "2026-11-04T10:00:00Z",
      ended_at: "2026-11-04T10:45:00Z",
      is_all_day: false,
      has_recurrence_rules: true,
      calendar_color: "#5B5BD6",
      recurrence_series_id: "series-sprint",
    },
    "event-2": {
      id: "event-2",
      title: "1:1 with Priya",
      started_at: "2026-11-04T16:30:00Z",
      ended_at: "2026-11-04T17:30:00Z",
      is_all_day: false,
      has_recurrence_rules: false,
      calendar_color: "#74BD00",
    },
    "event-allday": {
      id: "event-allday",
      title: "Company Holiday",
      started_at: "2026-11-05T00:00:00Z",
      is_all_day: true,
      has_recurrence_rules: false,
      calendar_color: "#C93B3B",
    },
  },
  sessionIdsByDate: {},
  sessionsById: {},
};

const standaloneNotes = {
  sessionIdsByDate: {
    "2026-11-04": ["note-1"],
    "2026-11-05": ["note-2"],
    "2026-11-09": ["note-outside-week"],
  },
  sessionsById: {
    "note-1": {
      title: "Standalone ideas",
      created_at: "2026-11-04T18:00:00Z",
    },
    "note-2": {
      title: "Thursday notes",
      created_at: "2026-11-05T08:00:00Z",
    },
    "note-outside-week": {
      title: "Next week notes",
      created_at: "2026-11-09T10:00:00Z",
    },
  },
};

describe("WeekView", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.now = new Date("2026-11-04T10:00:00Z");
    vi.stubGlobal(
      "ResizeObserver",
      class {
        observe() {}
        unobserve() {}
        disconnect() {}
      },
    );
  });

  afterEach(() => {
    cleanup();
    vi.unstubAllGlobals();
  });

  it("renders 7 day headers and time gutter labels", () => {
    render(<WeekView days={mockDays} calendarData={mockCalendarData} />);

    expect(screen.getByText("Mon")).toBeTruthy();
    expect(screen.getByText("Wed")).toBeTruthy();
    expect(screen.getByText("Sun")).toBeTruthy();

    expect(screen.getByText("9 AM")).toBeTruthy();
    expect(screen.getByText("10 AM")).toBeTruthy();
    expect(screen.getByText("12 PM")).toBeTruthy();
  });

  it("renders quiet rounded event blocks with title, duration, and repeat glyph", () => {
    render(<WeekView days={mockDays} calendarData={mockCalendarData} />);

    expect(screen.getByText("Sprint Planning")).toBeTruthy();
    expect(screen.getByText(/45 min/)).toBeTruthy();
    expect(screen.getByTitle("Recurring event")).toBeTruthy();

    expect(screen.getByText("1:1 with Priya")).toBeTruthy();
    expect(screen.getByText(/60 min/)).toBeTruthy();
  });

  it("renders all-day events row when all-day events exist", () => {
    render(<WeekView days={mockDays} calendarData={mockCalendarData} />);

    expect(screen.getByText("All day")).toBeTruthy();
    expect(screen.getByText("Company Holiday")).toBeTruthy();
  });

  it("keeps standalone notes visible without any calendar events", () => {
    render(
      <WeekView
        days={mockDays}
        calendarData={{
          eventIdsByDate: {},
          eventsById: {},
          ...standaloneNotes,
        }}
      />,
    );

    expect(screen.getByText("Notes")).toBeTruthy();
    expect(
      screen.getByRole("button", { name: /standalone ideas/i }),
    ).toBeTruthy();
    expect(
      screen.getByRole("button", { name: /thursday notes/i }),
    ).toBeTruthy();
    expect(screen.queryByText("Next week notes")).toBeNull();
    expect(screen.queryByText("All day")).toBeNull();
  });

  it("reuses standalone note opening and delete context-menu actions alongside events", async () => {
    render(
      <WeekView
        days={mockDays}
        calendarData={{ ...mockCalendarData, ...standaloneNotes }}
      />,
    );

    expect(screen.getByText("Sprint Planning")).toBeTruthy();
    expect(screen.getByText("Company Holiday")).toBeTruthy();
    const note = screen.getByRole("button", { name: /standalone ideas/i });
    fireEvent.click(note);
    fireEvent.click(await screen.findByRole("button", { name: "Open note" }));

    expect(mocks.openCurrent).toHaveBeenCalledWith({
      type: "sessions",
      id: "note-1",
    });
    expect(mocks.getOrCreateSessionForEventId).not.toHaveBeenCalled();

    fireEvent.contextMenu(note);
    const calls = mocks.showContextMenu.mock.calls;
    const menu = calls[calls.length - 1]?.[0] ?? [];
    const deleteItem = menu.find(
      (item) => "id" in item && item.id === "delete",
    );
    expect(deleteItem).toMatchObject({ text: "Delete Note" });
    if (!deleteItem || !("action" in deleteItem)) {
      throw new Error("Missing note delete action");
    }
    deleteItem.action();

    expect(mocks.deleteSession).toHaveBeenCalledWith("note-1", undefined);
    expect(mocks.ignoreEvent).not.toHaveBeenCalled();
    expect(mocks.ignoreSeries).not.toHaveBeenCalled();
  });

  it("preserves manual scrolling across minute ticks and same-week rerenders", () => {
    const { container, rerender } = render(
      <WeekView days={mockDays} calendarData={mockCalendarData} />,
    );
    const scroller = container.firstElementChild as HTMLDivElement;
    const scrollTo = vi.fn();
    scroller.scrollTo = scrollTo;
    const nextWeek = mockDays.map(
      (day) => new Date(day.getTime() + 7 * 24 * 60 * 60 * 1000),
    );

    rerender(<WeekView days={nextWeek} calendarData={mockCalendarData} />);
    rerender(<WeekView days={mockDays} calendarData={mockCalendarData} />);
    expect(scrollTo).toHaveBeenCalledExactlyOnceWith({
      top: 128,
      behavior: "instant",
    });

    fireEvent.scroll(scroller, { target: { scrollTop: 700 } });
    mocks.now = new Date("2026-11-04T10:01:00Z");
    rerender(
      <WeekView
        days={mockDays.map((day) => new Date(day))}
        calendarData={mockCalendarData}
      />,
    );
    expect(scroller.scrollTop).toBe(700);
    expect(scrollTo).toHaveBeenCalledTimes(1);

    const earlierEvent = {
      ...mockCalendarData,
      eventsById: {
        ...mockCalendarData.eventsById,
        "event-1": {
          ...mockCalendarData.eventsById["event-1"],
          started_at: "2026-11-04T06:00:00Z",
        },
      },
    };
    rerender(<WeekView days={mockDays} calendarData={earlierEvent} />);
    expect(scroller.scrollTop).toBe(700);
    expect(scrollTo).toHaveBeenCalledTimes(1);

    rerender(<WeekView days={nextWeek} calendarData={mockCalendarData} />);
    rerender(<WeekView days={mockDays} calendarData={mockCalendarData} />);
    expect(scrollTo).toHaveBeenCalledTimes(2);
  });

  it("keeps overlapping timed events in separate columns", () => {
    render(
      <WeekView
        days={mockDays}
        calendarData={{
          ...mockCalendarData,
          eventsById: {
            ...mockCalendarData.eventsById,
            "event-2": {
              ...mockCalendarData.eventsById["event-2"],
              started_at: "2026-11-04T10:15:00Z",
              ended_at: "2026-11-04T11:15:00Z",
            },
          },
        }}
      />,
    );

    const first = screen.getByRole("button", { name: /sprint planning/i });
    const second = screen.getByRole("button", { name: /1:1 with priya/i });
    expect(first.style.left).not.toBe(second.style.left);
    expect(first.style.width).toBe("calc(50% - 4px)");
    expect(second.style.width).toBe("calc(50% - 4px)");
  });

  it("clicking an event block calls getOrCreateSessionForEventId and opens session", async () => {
    render(<WeekView days={mockDays} calendarData={mockCalendarData} />);

    const eventBlock = screen.getByRole("button", { name: /sprint planning/i });
    fireEvent.click(eventBlock);

    expect(mocks.getOrCreateSessionForEventId).toHaveBeenCalledWith(
      "event-1",
      "Sprint Planning",
    );
    await vi.waitFor(() => {
      expect(mocks.openCurrent).toHaveBeenCalledWith({
        type: "sessions",
        id: "session-week-1",
      });
    });
  });
}, 20000);

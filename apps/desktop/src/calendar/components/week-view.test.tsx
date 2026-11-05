import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  openCurrent: vi.fn(),
  getOrCreateSessionForEventId: vi.fn(async () => "session-week-1"),
  ignoreEvent: vi.fn(),
  ignoreSeries: vi.fn(),
}));

vi.mock("@lingui/react/macro", () => ({
  Trans: ({ children }: { children: ReactNode }) => <>{children}</>,
  useLingui: () => ({
    t: (input: TemplateStringsArray | string) =>
      typeof input === "string" ? input : input[0],
  }),
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
  useNativeContextMenu: () => () => {},
}));

const mockNow = new Date("2026-11-04T10:00:00Z");

vi.mock("~/calendar/hooks", () => ({
  toTz: (iso: string) => new Date(iso),
  useNow: () => mockNow,
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

describe("WeekView", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  afterEach(() => {
    cleanup();
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

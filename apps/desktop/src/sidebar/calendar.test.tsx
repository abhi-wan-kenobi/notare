import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  openCurrent: vi.fn(),
  openNew: vi.fn(),
  getOrCreateSessionForEventId: vi.fn(async () => "session-123"),
  ignoreEvent: vi.fn(),
  ignoreSeries: vi.fn(),
  nativeContextMenus: [] as Array<
    Array<{ id: string; text?: string; action?: () => void }>
  >,
}));

vi.mock("@lingui/react/macro", () => ({
  Trans: ({ children }: { children: ReactNode }) => <>{children}</>,
  useLingui: () => ({
    t: (input: TemplateStringsArray | string) =>
      typeof input === "string" ? input : input[0],
  }),
}));

vi.mock("./custom-sidebar-header", () => ({
  CustomSidebarHeader: ({ title }: { title: ReactNode }) => <div>{title}</div>,
}));

vi.mock("~/store/zustand/tabs", () => ({
  useTabs: Object.assign(
    (selector: (state: unknown) => unknown) =>
      selector({
        openCurrent: mocks.openCurrent,
        openNew: mocks.openNew,
      }),
    {
      getState: () => ({
        openCurrent: mocks.openCurrent,
        openNew: mocks.openNew,
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
  useNativeContextMenu: (items: unknown) => () => {
    mocks.nativeContextMenus.push(
      items as Array<{ id: string; text?: string; action?: () => void }>,
    );
  },
}));

const mockNow = new Date("2026-11-04T10:00:00Z");

vi.mock("~/calendar/hooks", () => ({
  toTz: (iso: string) => new Date(iso),
  useNow: () => mockNow,
  useTimezone: () => "UTC",
  useWeekStartsOn: () => 1,
  useCalendarData: () => ({
    eventIdsByDate: {
      "2026-11-04": ["event-1", "event-2"],
    },
    eventsById: {
      "event-1": {
        id: "event-1",
        title: "Q4 Launch Sync",
        started_at: "2026-11-04T10:00:00Z",
        ended_at: "2026-11-04T10:45:00Z",
        is_all_day: false,
        calendar_color: "#5B5BD6",
        recurrence_series_id: "series-launch",
      },
      "event-2": {
        id: "event-2",
        title: "Infrastructure review",
        started_at: "2026-11-04T14:00:00Z",
        ended_at: "2026-11-04T15:00:00Z",
        is_all_day: false,
        calendar_color: "#74BD00",
      },
    },
    sessionIdsByDate: {},
    sessionsById: {},
  }),
}));

import { CalendarNav, CalendarSidebar } from "./calendar";

import { useCalendarSelection } from "~/calendar/selection";

describe("CalendarSidebar / CalendarNav", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useCalendarSelection.getState().selectDate(mockNow);
  });

  afterEach(() => {
    cleanup();
  });

  it("exports CalendarSidebar alias matching CalendarNav", () => {
    expect(CalendarSidebar).toBe(CalendarNav);
  });

  it("renders mini month, weekday labels, and selected day events", () => {
    render(<CalendarNav />);

    expect(screen.getByText("November 2026")).toBeTruthy();
    expect(screen.getByText("Mon")).toBeTruthy();
    expect(screen.getByText("Wed")).toBeTruthy();

    expect(screen.getByText("Wednesday, November 4")).toBeTruthy();
    expect(screen.getByText("Q4 Launch Sync")).toBeTruthy();
    expect(screen.getByText("Infrastructure review")).toBeTruthy();
    expect(screen.getByTitle("Recurring event")).toBeTruthy();
  });

  it("clicking an event opens or creates its session", async () => {
    render(<CalendarNav />);

    const eventButton = screen.getByRole("button", {
      name: /q4 launch sync/i,
    });
    fireEvent.click(eventButton);

    expect(mocks.getOrCreateSessionForEventId).toHaveBeenCalledWith(
      "event-1",
      "Q4 Launch Sync",
    );
    await vi.waitFor(() => {
      expect(mocks.openCurrent).toHaveBeenCalledWith({
        type: "sessions",
        id: "session-123",
      });
    });
  });

  it("clicking Manage calendars opens settings integrations tab", () => {
    render(<CalendarNav />);

    const manageBtn = screen.getByRole("button", { name: /manage calendars/i });
    fireEvent.click(manageBtn);

    expect(mocks.openNew).toHaveBeenCalledWith({
      type: "settings",
      state: { tab: "integrations" },
    });
  });

  it("clicking another day updates selection", () => {
    render(<CalendarNav />);

    const day5 = screen.getByRole("button", { name: /thursday, november 5/i });
    fireEvent.click(day5);

    expect(useCalendarSelection.getState().selectedDate).toBe("2026-11-05");
  });
}, 20000);

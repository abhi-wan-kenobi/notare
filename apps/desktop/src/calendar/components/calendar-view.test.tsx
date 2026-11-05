import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  openNew: vi.fn(),
  createSession: vi.fn(async () => "new-session-123"),
  zoom: "week",
  zoomBy: vi.fn(),
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
        openNew: mocks.openNew,
      }),
    {
      getState: () => ({
        openNew: mocks.openNew,
      }),
    },
  ),
}));

vi.mock("~/session/queries", () => ({
  createSession: mocks.createSession,
  getOrCreateSessionForEventId: vi.fn(async () => "session-1"),
}));

vi.mock("~/calendar/zoom", () => ({
  ZOOM_LEVELS: ["month", "week", "3day", "day"],
  useCalendarZoom: () => ({
    zoom: mocks.zoom,
    zoomBy: mocks.zoomBy,
  }),
  resolveCalendarLayout: (zoom: string) => ({
    isMonthView: zoom === "month",
    cols: 7,
  }),
  createPinchAccumulator: () => () => false,
}));

vi.mock("./context", () => ({
  useSync: () => ({
    canSync: false,
    syncRange: vi.fn(),
    scheduleSync: vi.fn(),
    cancelDebouncedSync: vi.fn(),
    status: "idle",
  }),
}));

vi.mock("~/calendar/hooks", () => ({
  toTz: (iso: string) => new Date(iso),
  useNow: () => new Date("2026-11-04T10:00:00Z"),
  useTimezone: () => "UTC",
  useWeekStartsOn: () => 1,
  useEnabledCalendars: () => [],
  useCalendarData: () => ({
    eventIdsByDate: {},
    eventsById: {},
    sessionIdsByDate: {},
    sessionsById: {},
  }),
}));

class MockResizeObserver {
  observe = vi.fn();
  unobserve = vi.fn();
  disconnect = vi.fn();
}
globalThis.ResizeObserver =
  MockResizeObserver as unknown as typeof ResizeObserver;

vi.mock("~/calendar/ignored-events", () => ({
  useIgnoredEvents: () => ({
    ignoreEvent: vi.fn(),
    ignoreSeries: vi.fn(),
  }),
}));

vi.mock("~/shared/hooks/useNativeContextMenu", () => ({
  useNativeContextMenu: () => () => {},
}));

import { CalendarView } from "./calendar-view";

describe("CalendarView", () => {
  let client: QueryClient;

  beforeEach(() => {
    vi.clearAllMocks();
    mocks.zoom = "week";
    client = new QueryClient({
      defaultOptions: { queries: { retry: false } },
    });
  });

  afterEach(() => {
    cleanup();
    client.clear();
  });

  it("renders New meeting button and calls createSession on click", async () => {
    render(
      <QueryClientProvider client={client}>
        <CalendarView />
      </QueryClientProvider>,
    );

    const newMeetingBtn = screen.getByRole("button", { name: /new meeting/i });
    expect(newMeetingBtn).toBeTruthy();

    fireEvent.click(newMeetingBtn);

    expect(mocks.createSession).toHaveBeenCalled();
    await vi.waitFor(() => {
      expect(mocks.openNew).toHaveBeenCalledWith({
        type: "sessions",
        id: "new-session-123",
      });
    });
  });

  it("renders WeekView when zoom is week", () => {
    mocks.zoom = "week";

    render(
      <QueryClientProvider client={client}>
        <CalendarView />
      </QueryClientProvider>,
    );

    expect(screen.getByTestId("calendar-zoom-level").textContent).toBe("Week");
    expect(screen.getByText("Mon")).toBeTruthy();
    expect(screen.getByText("Wed")).toBeTruthy();
  });

  it("renders month grid when zoom is month", () => {
    mocks.zoom = "month";

    render(
      <QueryClientProvider client={client}>
        <CalendarView />
      </QueryClientProvider>,
    );

    expect(screen.getByTestId("calendar-zoom-level").textContent).toBe("Month");
  });
}, 20000);

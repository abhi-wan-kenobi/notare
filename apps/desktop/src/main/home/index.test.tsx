import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { TZDate } from "@hypr/utils";

import { TabContentHome } from "./index";

import { openSurfaceTab } from "~/sidebar/surface-navigation";

const mocks = vi.hoisted(() => ({
  eventsTable: {} as Record<string, Record<string, unknown>>,
  sessions: [] as { id: string; title: string; created_at: string }[],
  sessionLinks: [] as {
    session_id: string;
    event_id: string;
    external_event_id: string;
    series_id?: string;
  }[],
  openActionItemsCount: 0,
  sessionsNeedingSummaryCount: 0,
  timezone: "America/Los_Angeles",
  openCurrent: vi.fn(),
  openNew: vi.fn(),
  getOrCreateEventSession: vi.fn(async () => "prepared-session"),
}));

vi.mock("~/shared/main", () => ({
  StandardContentWrapper: ({ children }: { children: React.ReactNode }) => (
    <div>{children}</div>
  ),
}));

vi.mock("~/shared/dock", () => ({
  Dock: () => <div data-testid="dock" />,
}));

vi.mock("~/calendar/hooks", () => ({
  useNow: () => new TZDate(new Date(), mocks.timezone),
  useTimezone: () => mocks.timezone,
  useWeekStartsOn: () => 0,
  toTz: (date: string | Date, tz?: string) => {
    const parsedDate = typeof date === "string" ? new Date(date) : date;
    return tz ? new TZDate(parsedDate, tz) : parsedDate;
  },
}));

vi.mock("~/calendar/queries", () => ({
  parseEventParticipants: () => [],
  useTimelineEventsTable: () => mocks.eventsTable,
}));

vi.mock("~/session/queries", () => ({
  getOrCreateSessionForEventId: mocks.getOrCreateEventSession,
  useSessionSummaries: () => mocks.sessions,
}));

vi.mock("~/sidebar/surface-navigation", () => ({
  openSurfaceTab: vi.fn(),
}));

vi.mock("~/db", () => ({
  useLiveQuery: ({ sql }: { sql: string }) => {
    if (sql.includes("FROM sessions AS session")) {
      return { data: mocks.sessionsNeedingSummaryCount };
    }
    if (sql.includes("FROM sessions")) return { data: mocks.sessionLinks };
    if (sql.includes("FROM events")) return { data: {} };
    if (sql.includes("FROM action_items")) {
      return { data: mocks.openActionItemsCount };
    }
    throw new Error(`Unexpected Home live query: ${sql}`);
  },
}));

type MockTabsState = {
  openCurrent: typeof mocks.openCurrent;
  openNew: typeof mocks.openNew;
};

vi.mock("~/store/zustand/tabs", () => ({
  useTabs: (selector: (state: MockTabsState) => unknown) => selector(state),
}));

const state: MockTabsState = {
  openCurrent: mocks.openCurrent,
  openNew: mocks.openNew,
};

describe("TabContentHome", () => {
  beforeEach(() => {
    cleanup();
    mocks.eventsTable = {};
    mocks.sessions = [];
    mocks.sessionLinks = [];
    mocks.openActionItemsCount = 0;
    mocks.timezone = "America/Los_Angeles";
    mocks.openCurrent.mockClear();
    mocks.sessionsNeedingSummaryCount = 0;
    vi.mocked(openSurfaceTab).mockClear();
    mocks.getOrCreateEventSession.mockClear();
  });

  afterEach(() => {
    cleanup();
    vi.useRealTimers();
  });

  const renderHome = () =>
    render(<TabContentHome tab={{ type: "empty" } as never} />);

  it("shows the need-a-summary count and opens the matching surface when clicked", () => {
    mocks.sessionsNeedingSummaryCount = 2;

    renderHome();

    const needsSummary = screen.getByRole("button", {
      name: "2 need a summary",
    });
    fireEvent.click(needsSummary);

    expect(openSurfaceTab).toHaveBeenCalledWith("meetings");
  });

  it("opens Calendar from the scheduled-today count", () => {
    const now = new Date();
    mocks.eventsTable = {
      "evt-1": {
        title: "Today's sync",
        started_at: now.toISOString(),
      },
    };

    renderHome();
    fireEvent.click(screen.getByRole("button", { name: "1 scheduled today" }));

    expect(openSurfaceTab).toHaveBeenCalledWith("calendar");
  });

  it("opens Meetings from the recorded-this-week count", () => {
    mocks.sessions = [
      { id: "s1", title: "Note 1", created_at: new Date().toISOString() },
    ];
    renderHome();
    fireEvent.click(
      screen.getByRole("button", { name: "1 recorded this week" }),
    );
    expect(openSurfaceTab).toHaveBeenCalledWith("meetings");
  });

  it("opens Action items from the open-actions count", () => {
    mocks.openActionItemsCount = 3;
    renderHome();
    fireEvent.click(
      screen.getByRole("button", { name: "3 open action items" }),
    );
    expect(openSurfaceTab).toHaveBeenCalledWith("action-items");
  });
  it("counts today's events in the selected timezone", () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-11-05T08:00:00.000Z"));
    mocks.eventsTable = {
      "evt-1": {
        title: "Evening meeting",
        started_at: "2026-11-06T02:00:00.000Z",
      },
    };

    renderHome();

    expect(
      screen.getByRole("button", { name: "1 scheduled today" }),
    ).toBeTruthy();
    expect(screen.getByText("Evening meeting")).toBeTruthy();
  });
  it("keeps future events visible when the selected timezone is ahead of UTC", () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-11-08T08:30:00.000Z"));
    mocks.timezone = "Asia/Kolkata";
    mocks.eventsTable = {
      "evt-1": {
        title: "Soon in Kolkata",
        started_at: "2026-11-08T08:45:00.000Z",
      },
    };

    renderHome();

    expect(screen.getByText("Soon in Kolkata")).toBeTruthy();
  });
  it("uses the selected timezone for the recorded-this-week boundary", () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-11-08T08:30:00.000Z"));
    mocks.sessions = [
      {
        id: "s-before-week",
        title: "Previous local week",
        created_at: "2026-11-08T07:30:00.000Z",
      },
    ];

    renderHome();

    expect(
      screen.getByRole("button", { name: "0 recorded this week" }),
    ).toBeTruthy();
  });
  it("includes a session created at the selected timezone's week start", () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-11-08T08:30:00.000Z"));
    mocks.sessions = [
      {
        id: "s-at-week-start",
        title: "Sunday note",
        created_at: "2026-11-08T08:00:00.000Z",
      },
    ];

    renderHome();

    expect(
      screen.getByRole("button", { name: "1 recorded this week" }),
    ).toBeTruthy();
  });

  it("shows recent note dates in the selected timezone", () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-11-08T08:30:00.000Z"));
    mocks.sessions = [
      {
        id: "s-local-date",
        title: "Saturday note",
        created_at: "2026-11-08T07:30:00.000Z",
      },
    ];

    renderHome();

    expect(screen.getByText(/ago/)).toBeTruthy();
  });

  it("refreshes today's summary when Home rerenders after midnight", () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-11-08T08:30:00.000Z"));
    mocks.eventsTable = {
      "evt-1": {
        title: "Today event",
        started_at: "2026-11-08T09:00:00.000Z",
      },
    };

    const { rerender } = renderHome();
    expect(
      screen.getByRole("button", { name: "1 scheduled today" }),
    ).toBeTruthy();

    vi.setSystemTime(new Date("2026-11-09T08:30:00.000Z"));
    rerender(<TabContentHome tab={{ type: "empty" } as never} />);

    expect(
      screen.getByRole("button", { name: "0 scheduled today" }),
    ).toBeTruthy();
  });

  it("only lists future events in Coming up, sorted by start time", () => {
    const now = Date.now();
    mocks.eventsTable = {
      past: {
        title: "Past meeting",
        started_at: new Date(now - 60_000).toISOString(),
      },
      later: {
        title: "Later meeting",
        started_at: new Date(now + 2 * 60_000).toISOString(),
      },
      soon: {
        title: "Soon meeting",
        started_at: new Date(now + 60_000).toISOString(),
      },
    };

    renderHome();

    expect(screen.queryByText("Past meeting")).toBeNull();
    const titles = screen.getAllByText(/meeting$/).map((el) => el.textContent);
    expect(titles).toEqual(["Soon meeting", "Later meeting"]);
  });

  it("shows Open notes for an event with a linked session and prepares one linked to the event otherwise", async () => {
    const now = Date.now();
    mocks.eventsTable = {
      linked: {
        title: "Has a note",
        started_at: new Date(now + 60_000).toISOString(),
      },
      unlinked: {
        title: "No note yet",
        started_at: new Date(now + 120_000).toISOString(),
      },
    };
    mocks.sessionLinks = [
      { session_id: "session-1", event_id: "linked", external_event_id: "" },
    ];

    renderHome();

    const rows = screen.getAllByText(/^(Has a note|No note yet)$/);
    expect(rows[0].textContent).toBe("Has a note");
    expect(rows[1].textContent).toBe("No note yet");

    fireEvent.click(screen.getByRole("button", { name: "Open notes" }));
    expect(mocks.openCurrent).toHaveBeenCalledWith({
      type: "sessions",
      id: "session-1",
    });

    fireEvent.click(screen.getByRole("button", { name: "Prepare" }));
    await waitFor(() =>
      expect(mocks.getOrCreateEventSession).toHaveBeenCalledWith(
        "unlinked",
        "No note yet",
      ),
    );
    await waitFor(() =>
      expect(mocks.openCurrent).toHaveBeenCalledWith({
        type: "sessions",
        id: "prepared-session",
      }),
    );
  });

  it("lists recent notes and opens one on click", () => {
    mocks.sessions = [
      { id: "s1", title: "First note", created_at: new Date().toISOString() },
      { id: "s2", title: "Second note", created_at: new Date().toISOString() },
    ];

    renderHome();

    fireEvent.click(screen.getByText("First note"));

    expect(mocks.openCurrent).toHaveBeenCalledWith({
      type: "sessions",
      id: "s1",
    });
  });

  it("renders repeat glyph for series members in recent notes", () => {
    mocks.sessions = [
      {
        id: "s-series",
        title: "Series note",
        created_at: new Date().toISOString(),
      },
    ];
    mocks.sessionLinks = [
      {
        session_id: "s-series",
        event_id: "",
        external_event_id: "",
        series_id: "series-1",
      },
    ];

    renderHome();

    const noteButton = screen.getByText("Series note").closest("button");
    expect(noteButton?.querySelector("svg")).toBeTruthy();
  });

  it("mounts the dock", () => {
    renderHome();

    expect(screen.getByTestId("dock")).toBeTruthy();
  });
});

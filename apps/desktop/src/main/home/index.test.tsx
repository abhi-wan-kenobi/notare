import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  eventsTable: {} as Record<string, Record<string, unknown>>,
  sessions: [] as { id: string; title: string; created_at: string }[],
  sessionLinks: [] as {
    session_id: string;
    event_id: string;
    external_event_id: string;
  }[],
  openActionItemsCount: 0,
  timezone: "America/Los_Angeles",
  openCurrent: vi.fn(),
  openNew: vi.fn(),
  newNote: vi.fn(),
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
  useTimezone: () => mocks.timezone,
  useWeekStartsOn: () => 0,
  toTz: (date: string | Date, tz?: string) =>
    tz
      ? new Date(new Date(date).toLocaleString("en-US", { timeZone: tz }))
      : new Date(date),
}));

vi.mock("~/calendar/queries", () => ({
  useTimelineEventsTable: () => mocks.eventsTable,
}));

vi.mock("~/session/queries", () => ({
  useSessionSummaries: () => mocks.sessions,
}));

vi.mock("~/shared/useNewNote", () => ({
  useNewNote: () => mocks.newNote,
}));

vi.mock("~/sidebar/surface-navigation", () => ({
  openSurfaceTab: vi.fn(),
}));

let queryCallIndex = 0;
vi.mock("~/db", () => ({
  useLiveQuery: () => {
    // Two useLiveQuery calls per render, in declaration order: session
    // links, then the open action items count.
    const index = queryCallIndex % 2;
    queryCallIndex += 1;
    return index === 0
      ? { data: mocks.sessionLinks }
      : { data: mocks.openActionItemsCount };
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

import { TabContentHome } from "./index";

describe("TabContentHome", () => {
  beforeEach(() => {
    cleanup();
    queryCallIndex = 0;
    mocks.eventsTable = {};
    mocks.sessions = [];
    mocks.sessionLinks = [];
    mocks.openActionItemsCount = 0;
    mocks.timezone = "America/Los_Angeles";
    mocks.openCurrent.mockClear();
    mocks.newNote.mockClear();
  });

  afterEach(() => {
    cleanup();
  });

  const renderHome = () =>
    render(<TabContentHome tab={{ type: "empty" } as never} />);

  it("shows the one-line summary from scheduled, recorded, and open-action counts", () => {
    const now = new Date();
    mocks.eventsTable = {
      "evt-1": {
        title: "Today's sync",
        started_at: now.toISOString(),
      },
    };
    mocks.sessions = [
      { id: "s1", title: "Recent note", created_at: now.toISOString() },
    ];
    mocks.openActionItemsCount = 3;

    renderHome();

    expect(
      screen.getByText(
        /1 scheduled today · 1 recorded this week · 3 open actions/,
      ),
    ).toBeTruthy();
  });
  it("counts today's events in the selected timezone", () => {
    vi.setSystemTime(new Date("2026-11-05T08:00:00.000Z"));
    mocks.eventsTable = {
      "evt-1": {
        title: "Evening meeting",
        started_at: "2026-11-06T02:00:00.000Z",
      },
    };

    renderHome();

    expect(screen.getByText(/1 scheduled today/)).toBeTruthy();
    vi.useRealTimers();
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

  it("shows Open notes for an event with a linked session and Prepare otherwise", () => {
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
    expect(mocks.newNote).toHaveBeenCalled();
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

  it("mounts the dock", () => {
    renderHome();

    expect(screen.getByTestId("dock")).toBeTruthy();
  });
});

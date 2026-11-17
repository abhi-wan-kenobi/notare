import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

type MeetingMockRow = {
  id: string;
  title: string;
  created_at: string;
  started_at: string;
  ended_at: string;
  event_json: string;
  series_id: string;
  transcript_count: number;
  action_count: number;
  attendee_count: number;
  attendees: string;
};

const mocks = vi.hoisted(() => ({
  meetings: [] as MeetingMockRow[],
  openCurrent: vi.fn(),
  setState: vi.fn(),
  now: new Date() as Date,
  currentTab: {
    type: "meetings",
    state: undefined as { filter?: string } | undefined,
  },
  tabs: [] as unknown[],
}));

vi.mock("~/shared/main", () => ({
  StandardContentWrapper: ({ children }: { children: React.ReactNode }) => (
    <div>{children}</div>
  ),
}));

vi.mock("~/db", () => ({
  useLiveQuery: () => ({ data: mocks.meetings }),
}));

vi.mock("~/store/zustand/tabs", () => ({
  useTabs: Object.assign(
    (selector: (state: typeof mocks) => unknown) => selector(mocks),
    {
      setState: (updater: (state: typeof mocks) => Partial<typeof mocks>) => {
        const next = updater(mocks);
        Object.assign(mocks, next);
        mocks.setState(next);
      },
    },
  ),
}));

vi.mock("~/calendar/hooks", () => ({
  useNow: () => mocks.now,
}));

import { TabContentMeetings } from "./index";

describe("TabContentMeetings", () => {
  beforeEach(() => {
    mocks.meetings = [];
    mocks.openCurrent.mockClear();
    mocks.setState.mockClear();
    mocks.currentTab = { type: "meetings", state: undefined };
    mocks.now = new Date();
    mocks.tabs = [mocks.currentTab];
  });

  afterEach(() => {
    cleanup();
  });

  it("renders empty state when there are no meetings", () => {
    render(<TabContentMeetings />);
    expect(screen.getByText("Meetings")).toBeDefined();
    expect(
      screen.getByText("Your conversations, notes, and what comes next."),
    ).toBeDefined();
    expect(screen.getByText("No meetings found.")).toBeDefined();
  });

  it("renders meetings grouped by period with meta, recurrence, and status", () => {
    const today = mocks.now.toISOString();
    mocks.meetings = [
      {
        id: "sess-today",
        title: "Today Sync",
        created_at: today,
        started_at: today,
        ended_at: today,
        event_json: "",
        series_id: "series-weekly",
        transcript_count: 5,
        action_count: 3,
        attendee_count: 4,
        attendees: "Alice, Bob, David",
      },
      {
        id: "sess-notes-only",
        title: "Design Critique",
        created_at: today,
        started_at: today,
        ended_at: "",
        event_json: "",
        series_id: "",
        transcript_count: 0,
        action_count: 0,
        attendee_count: 2,
        attendees: "Maya, David",
      },
    ];

    render(<TabContentMeetings />);

    expect(screen.getByText("Today")).toBeDefined();
    expect(screen.getByText("Today Sync")).toBeDefined();
    expect(screen.getByText("Design Critique")).toBeDefined();
    expect(screen.getByText(/Alice, Bob, David/)).toBeDefined();
    expect(screen.getByText("4 attendees")).toBeDefined();
    expect(screen.getAllByText("Transcribed").length).toBeGreaterThanOrEqual(2);
    expect(screen.getByText("3 open")).toBeDefined();
    expect(screen.getByText("Notes only")).toBeDefined();
    expect(screen.getAllByLabelText("Recurring").length).toBeGreaterThan(0);
  });

  it("filters meetings when clicking segmented filter buttons", () => {
    const now = Date.now();
    const futureDate = new Date(now + 86400000 * 2).toISOString();
    const pastDate = new Date(now - 86400000).toISOString();

    mocks.meetings = [
      {
        id: "sess-upcoming",
        title: "Future Roadmapping",
        created_at: pastDate,
        started_at: futureDate,
        ended_at: "",
        event_json: "",
        series_id: "",
        transcript_count: 0,
        action_count: 0,
        attendee_count: 3,
        attendees: "David, Bob",
      },
      {
        id: "sess-recorded",
        title: "Past Recorded",
        created_at: pastDate,
        started_at: pastDate,
        ended_at: pastDate,
        event_json: "",
        series_id: "series-123",
        transcript_count: 10,
        action_count: 1,
        attendee_count: 2,
        attendees: "Alice, Bob",
      },
    ];

    render(<TabContentMeetings />);

    expect(screen.getByText("Future Roadmapping")).toBeDefined();
    expect(screen.getByText("Past Recorded")).toBeDefined();

    // Click "Upcoming" filter
    const upcomingBtn = screen.getByRole("button", { name: "Upcoming" });
    fireEvent.click(upcomingBtn);

    expect(screen.getByText("Future Roadmapping")).toBeDefined();
    expect(screen.queryByText("Past Recorded")).toBeNull();

    // Click "Series" filter
    const seriesBtn = screen.getByRole("button", { name: "Series" });
    fireEvent.click(seriesBtn);

    expect(screen.queryByText("Future Roadmapping")).toBeNull();
    expect(screen.getByText("Past Recorded")).toBeDefined();

    // Verify filter state was updated in useTabs
    expect(mocks.setState).toHaveBeenCalled();
  });

  it("filters meetings using the search input", () => {
    const today = mocks.now.toISOString();
    mocks.meetings = [
      {
        id: "sess-1",
        title: "Infrastructure Review",
        created_at: today,
        started_at: today,
        ended_at: today,
        event_json: "",
        series_id: "",
        transcript_count: 2,
        action_count: 0,
        attendee_count: 1,
        attendees: "Priya",
      },
      {
        id: "sess-2",
        title: "Sprint Retrospective",
        created_at: today,
        started_at: today,
        ended_at: today,
        event_json: "",
        series_id: "",
        transcript_count: 1,
        action_count: 0,
        attendee_count: 2,
        attendees: "Bob, Alice",
      },
    ];

    render(<TabContentMeetings />);

    const searchInput = screen.getByLabelText("Search meetings");
    fireEvent.change(searchInput, { target: { value: "Infra" } });

    expect(screen.getByText("Infrastructure Review")).toBeDefined();
    expect(screen.queryByText("Sprint Retrospective")).toBeNull();

    // Search by attendee
    fireEvent.change(searchInput, { target: { value: "Bob" } });
    expect(screen.queryByText("Infrastructure Review")).toBeNull();
    expect(screen.getByText("Sprint Retrospective")).toBeDefined();
  });

  it("navigates to session when the row is clicked, with a single native row action", () => {
    const today = mocks.now.toISOString();
    mocks.meetings = [
      {
        id: "sess-click",
        title: "Clickable Meeting",
        created_at: today,
        started_at: today,
        ended_at: today,
        event_json: "",
        series_id: "",
        transcript_count: 1,
        action_count: 0,
        attendee_count: 1,
        attendees: "Alice",
      },
    ];

    render(<TabContentMeetings />);

    expect(screen.queryByRole("button", { name: "Open" })).toBeNull();

    const row = screen.getByRole("button", { name: /Clickable Meeting/ });
    fireEvent.click(row);

    expect(mocks.openCurrent).toHaveBeenCalledTimes(1);
    expect(mocks.openCurrent).toHaveBeenCalledWith({
      type: "sessions",
      id: "sess-click",
    });
  });

  it("picks up initial filter from tab state", () => {
    const today = mocks.now.toISOString();
    mocks.currentTab = {
      type: "meetings",
      state: { filter: "series" },
    };
    mocks.tabs = [mocks.currentTab];

    mocks.meetings = [
      {
        id: "sess-series",
        title: "Series Only",
        created_at: today,
        started_at: today,
        ended_at: today,
        event_json: "",
        series_id: "series-abc",
        transcript_count: 1,
        action_count: 0,
        attendee_count: 1,
        attendees: "Alice",
      },
      {
        id: "sess-non-series",
        title: "One-off",
        created_at: today,
        started_at: today,
        ended_at: today,
        event_json: "",
        series_id: "",
        transcript_count: 1,
        action_count: 0,
        attendee_count: 1,
        attendees: "Bob",
      },
    ];

    render(<TabContentMeetings />);

    expect(screen.getByText("Series Only")).toBeDefined();
    expect(screen.queryByText("One-off")).toBeNull();
  });

  it("owns vertical scrolling for an overflowing meetings list without nesting scrollers", () => {
    const base = new Date("2026-11-17T12:00:00.000Z").getTime();
    mocks.meetings = Array.from({ length: 30 }, (_, i) => {
      const iso = new Date(base - i * 86400000 * 40).toISOString();
      return {
        id: `sess-${i}`,
        title: `Meeting ${i}`,
        created_at: iso,
        started_at: iso,
        ended_at: iso,
        event_json: "",
        series_id: "",
        transcript_count: 1,
        action_count: 0,
        attendee_count: 1,
        attendees: "Alice",
      };
    });

    const { container } = render(<TabContentMeetings />);

    const scrollers = Array.from(
      container.querySelectorAll<HTMLElement>("div"),
    ).filter((el) => el.className.includes("overflow-y-auto"));
    expect(scrollers).toHaveLength(1);
    const scroller = scrollers[0]!;

    const firstRow = screen.getByRole("button", { name: /Meeting 0\b/ });
    const lastRow = screen.getByRole("button", { name: /Meeting 29\b/ });
    expect(scroller.contains(firstRow)).toBe(true);
    expect(scroller.contains(lastRow)).toBe(true);

    Object.defineProperty(scroller, "scrollHeight", {
      configurable: true,
      value: 4000,
    });
    Object.defineProperty(scroller, "clientHeight", {
      configurable: true,
      value: 400,
    });
    scroller.scrollTop = 250;
    fireEvent.scroll(scroller);
    expect(scroller.scrollTop).toBe(250);
  });

  it("shows a localized weekday/date on each row, with the year only for a prior-year meeting", () => {
    mocks.now = new Date("2026-11-17T12:00:00.000Z");
    mocks.meetings = [
      {
        id: "sess-current-year",
        title: "Current Year Sync",
        created_at: "2026-11-10T12:00:00.000Z",
        started_at: "2026-11-10T12:00:00.000Z",
        ended_at: "2026-11-10T12:30:00.000Z",
        event_json: "",
        series_id: "",
        transcript_count: 1,
        action_count: 0,
        attendee_count: 1,
        attendees: "Alice",
      },
      {
        id: "sess-prior-year",
        title: "Prior Year Kickoff",
        created_at: "2024-03-05T12:00:00.000Z",
        started_at: "2024-03-05T12:00:00.000Z",
        ended_at: "2024-03-05T12:30:00.000Z",
        event_json: "",
        series_id: "",
        transcript_count: 1,
        action_count: 0,
        attendee_count: 1,
        attendees: "Bob",
      },
    ];

    render(<TabContentMeetings />);

    expect(screen.getByText(/Tue, Nov 10/)).toBeDefined();
    expect(screen.queryByText(/Tue, Nov 10, 2026/)).toBeNull();
    expect(screen.getByText(/Tue, Mar 5, 2024/)).toBeDefined();
  });

  it("re-evaluates upcoming status as the clock advances past meeting start", () => {
    const meetingStart = "2026-11-17T15:00:00.000Z";
    mocks.now = new Date("2026-11-17T14:00:00.000Z");
    mocks.meetings = [
      {
        id: "sess-boundary",
        title: "Boundary Meeting",
        created_at: meetingStart,
        started_at: meetingStart,
        ended_at: "",
        event_json: "",
        series_id: "",
        transcript_count: 3,
        action_count: 0,
        attendee_count: 1,
        attendees: "Alice",
      },
    ];

    const { rerender } = render(<TabContentMeetings />);
    expect(screen.getAllByText("Upcoming")).toHaveLength(2);
    expect(screen.getAllByText("Transcribed")).toHaveLength(1);

    mocks.now = new Date("2026-11-17T16:00:00.000Z");
    rerender(<TabContentMeetings />);

    expect(screen.getAllByText("Upcoming")).toHaveLength(1);
    expect(screen.getAllByText("Transcribed")).toHaveLength(2);
  });

  it("re-groups a meeting from Today to Yesterday as the clock crosses local midnight", () => {
    const meetingLocal = new Date(2026, 10, 16, 23, 30, 0);
    mocks.now = new Date(2026, 10, 16, 23, 45, 0);
    mocks.meetings = [
      {
        id: "sess-midnight",
        title: "Late Night Standup",
        created_at: meetingLocal.toISOString(),
        started_at: meetingLocal.toISOString(),
        ended_at: meetingLocal.toISOString(),
        event_json: "",
        series_id: "",
        transcript_count: 1,
        action_count: 0,
        attendee_count: 1,
        attendees: "Alice",
      },
    ];

    const { rerender } = render(<TabContentMeetings />);
    expect(screen.getByText("Today")).toBeDefined();
    expect(screen.queryByText("Yesterday")).toBeNull();

    mocks.now = new Date(2026, 10, 17, 0, 15, 0);
    rerender(<TabContentMeetings />);

    expect(screen.queryByText("Today")).toBeNull();
    expect(screen.getByText("Yesterday")).toBeDefined();
  });
});

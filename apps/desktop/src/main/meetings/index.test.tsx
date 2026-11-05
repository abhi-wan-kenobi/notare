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

import { TabContentMeetings } from "./index";

describe("TabContentMeetings", () => {
  beforeEach(() => {
    mocks.meetings = [];
    mocks.openCurrent.mockClear();
    mocks.setState.mockClear();
    mocks.currentTab = { type: "meetings", state: undefined };
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
    const today = new Date().toISOString();
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
    expect(screen.getAllByText("Recorded").length).toBeGreaterThanOrEqual(2);
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
    const today = new Date().toISOString();
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

  it("navigates to session when row or open button is clicked", () => {
    const today = new Date().toISOString();
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

    const openBtn = screen.getByRole("button", { name: "Open" });
    fireEvent.click(openBtn);

    expect(mocks.openCurrent).toHaveBeenCalledWith({
      type: "sessions",
      id: "sess-click",
    });
  });

  it("picks up initial filter from tab state", () => {
    const today = new Date().toISOString();
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
});

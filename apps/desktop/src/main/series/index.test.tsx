import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  occurrences: [] as Array<{
    id: string;
    title: string;
    started_at: string;
    ended_at: string;
    created_at: string;
    date: string;
    duration_minutes: number;
    tldr: string;
    enhanced_note: string;
    open_item_count: number;
    event_json: string;
  }>,
  carryOver: [] as Array<{
    id: string;
    session_id: string;
    text: string;
    assignee_human_id: string;
    assignee_name: string;
    due_at: string;
    carried_from_session_id: string | null;
    source_title: string;
    source_started_at: string;
  }>,
  delta: {
    decisions: [] as Array<{ text: string; owner?: string }>,
    actionsClosed: [] as Array<{ id: string; text: string }>,
    attendeesJoined: [] as Array<{ name: string; email?: string }>,
    attendeesLeft: [] as Array<{ name: string; email?: string }>,
    topicsAdded: [] as string[],
    topicsRemoved: [] as string[],
  },
  openCurrent: vi.fn(),
  prepareSeriesOccurrence: vi.fn(),
  setActionItemStatus: vi.fn(),
}));

vi.mock("~/shared/main", () => ({
  StandardContentWrapper: ({ children }: { children: React.ReactNode }) => (
    <div>{children}</div>
  ),
}));

vi.mock("~/shared/dock", () => ({
  Dock: () => <div data-testid="dock" />,
}));

vi.mock("~/store/zustand/tabs", () => ({
  useTabs: (
    selector: (state: { openCurrent: typeof mocks.openCurrent }) => unknown,
  ) => selector({ openCurrent: mocks.openCurrent }),
}));

vi.mock("~/services/series", () => ({
  useSeries: () => ({ data: mocks.occurrences }),
  useSeriesCarryOver: () => ({ data: mocks.carryOver }),
  useSeriesDelta: () => mocks.delta,
  prepareSeriesOccurrence: (...args: unknown[]) =>
    mocks.prepareSeriesOccurrence(...args),
  parseParticipants: (eventJson?: string | null) => {
    if (!eventJson) return [];
    try {
      const parsed = JSON.parse(eventJson);
      return parsed.participants || [];
    } catch {
      return [];
    }
  },
}));

vi.mock("~/session/components/action-items/queries", () => ({
  setActionItemStatus: (...args: unknown[]) =>
    mocks.setActionItemStatus(...args),
}));

import { TabContentSeries } from "./index";

describe("TabContentSeries", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.occurrences = [
      {
        id: "sess-2",
        title: "Q4 Launch Sync",
        started_at: "2026-11-04T10:00:00.000Z",
        ended_at: "2026-11-04T10:45:00.000Z",
        created_at: "2026-11-04T09:50:00.000Z",
        date: "2026-11-04T10:00:00.000Z",
        duration_minutes: 45,
        tldr: "Release target set for December 1",
        enhanced_note: "## Release dates\nTarget set for Dec 1.",
        open_item_count: 2,
        event_json: JSON.stringify({
          participants: [
            { name: "Alice Sharma", email: "alice@example.com" },
            { name: "Bob Miller", email: "bob@example.com" },
          ],
        }),
      },
      {
        id: "sess-1",
        title: "Q4 Launch Sync",
        started_at: "2026-10-28T10:00:00.000Z",
        ended_at: "2026-10-28T10:45:00.000Z",
        created_at: "2026-10-28T09:50:00.000Z",
        date: "2026-10-28T10:00:00.000Z",
        duration_minutes: 45,
        tldr: "Token rotation verified",
        enhanced_note: "## Token rotation\nPassed verification.",
        open_item_count: 1,
        event_json: JSON.stringify({
          participants: [{ name: "Alice Sharma", email: "alice@example.com" }],
        }),
      },
    ];

    mocks.carryOver = [
      {
        id: "item-1",
        session_id: "sess-1",
        text: "Review token rotation error handling",
        assignee_human_id: "human-1",
        assignee_name: "Alice Sharma",
        due_at: "",
        carried_from_session_id: null,
        source_title: "Q4 Launch Sync",
        source_started_at: "2026-10-28T10:00:00.000Z",
      },
    ];

    mocks.delta = {
      decisions: [{ text: "Gateway access tokens capped at 24 hours" }],
      actionsClosed: [{ id: "act-1", text: "Complete audio loopback spike" }],
      attendeesJoined: [{ name: "Bob Miller" }],
      attendeesLeft: [],
      topicsAdded: ["Release dates"],
      topicsRemoved: ["Token rotation"],
    };
  });

  afterEach(() => {
    cleanup();
  });

  it("renders series header with cadence, attendees, and occurrences count", () => {
    render(
      <TabContentSeries
        tab={
          {
            type: "series",
            active: true,
            pinned: false,
            slotId: "main",
            state: { seriesId: "series-1" },
          } as never
        }
      />,
    );

    expect(screen.getByText("Recurring meeting series")).toBeDefined();
    expect(
      screen.getByRole("heading", { level: 1, name: "Q4 Launch Sync" }),
    ).toBeDefined();
    expect(screen.getAllByText(/2 meetings/).length).toBe(2);
    expect(screen.getByText(/Alice Sharma, Bob Miller/)).toBeDefined();
  });

  it("renders carried-over actions and toggles status on click", async () => {
    render(
      <TabContentSeries
        tab={
          {
            type: "series",
            active: true,
            pinned: false,
            slotId: "main",
            state: { seriesId: "series-1" },
          } as never
        }
      />,
    );

    expect(
      screen.getByText("Review token rotation error handling"),
    ).toBeDefined();
    expect(screen.getAllByText(/Alice Sharma/).length).toBeGreaterThan(0);
    const checkbox = screen.getByRole("checkbox");
    expect(checkbox).toBeDefined();
    fireEvent.click(checkbox);

    expect(mocks.setActionItemStatus).toHaveBeenCalledWith(
      "item-1",
      "done",
      expect.any(String),
    );
  });

  it("renders what changed since last time with decisions, actions, and topics", () => {
    render(
      <TabContentSeries
        tab={
          {
            type: "series",
            active: true,
            pinned: false,
            slotId: "main",
            state: { seriesId: "series-1" },
          } as never
        }
      />,
    );

    expect(
      screen.getByText("Gateway access tokens capped at 24 hours"),
    ).toBeDefined();
    expect(screen.getByText("Complete audio loopback spike")).toBeDefined();
    expect(screen.getByText(/Bob Miller joined this series/)).toBeDefined();
    expect(screen.getByText("Release dates")).toBeDefined();
    expect(screen.getByText("Token rotation")).toBeDefined();
  });

  it("renders timeline meetings and opens session on click", () => {
    render(
      <TabContentSeries
        tab={
          {
            type: "series",
            active: true,
            pinned: false,
            slotId: "main",
            state: { seriesId: "series-1" },
          } as never
        }
      />,
    );

    const buttons = screen.getAllByRole("button");
    const sessionButton = buttons.find((b) =>
      b.textContent?.includes("Release target set for December 1"),
    );
    expect(sessionButton).toBeDefined();

    fireEvent.click(sessionButton!);
    expect(mocks.openCurrent).toHaveBeenCalledWith({
      type: "sessions",
      id: "sess-2",
    });
  });

  it("calls prepareSeriesOccurrence when clicking Prepare next", async () => {
    mocks.prepareSeriesOccurrence.mockResolvedValueOnce("new-session-id");

    render(
      <TabContentSeries
        tab={
          {
            type: "series",
            active: true,
            pinned: false,
            slotId: "main",
            state: { seriesId: "series-1" },
          } as never
        }
      />,
    );

    const prepareBtn = screen.getByRole("button", { name: "Prepare next" });
    fireEvent.click(prepareBtn);

    expect(mocks.prepareSeriesOccurrence).toHaveBeenCalledWith("series-1");
  });
});

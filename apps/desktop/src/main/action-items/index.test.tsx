import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { AllActionItem, AllActionItemFilter } from "./queries";

const mocks = vi.hoisted(() => ({
  items: [] as AllActionItem[],
  doneItems: [] as AllActionItem[],
  openCount: 0,
  ownerUserId: "user-1",
  openCurrent: vi.fn(),
  setActionItemStatus: vi.fn().mockResolvedValue(undefined),
}));

vi.mock("~/shared/main", () => ({
  StandardContentWrapper: ({ children }: { children: React.ReactNode }) => (
    <div>{children}</div>
  ),
}));

vi.mock("~/shared/owner-user", () => ({
  useOwnerUserId: () => mocks.ownerUserId,
}));

vi.mock("~/session/components/action-items/queries", () => ({
  setActionItemStatus: (...args: unknown[]) =>
    mocks.setActionItemStatus(...args),
}));

vi.mock("./queries", () => ({
  useAllActionItems: (filter: AllActionItemFilter) => {
    if (filter === "done") {
      return { items: mocks.doneItems, isLoading: false };
    }
    if (filter === "open") {
      return {
        items: mocks.items.filter((i) => i.status !== "done"),
        isLoading: false,
      };
    }
    if (filter === "mine") {
      return {
        items: mocks.items.filter(
          (i) => i.status !== "done" && i.assignee_human_id === "user-1",
        ),
        isLoading: false,
      };
    }
    if (filter === "overdue") {
      return {
        items: mocks.items.filter(
          (i) =>
            i.status !== "done" &&
            i.due_at &&
            i.due_at < new Date().toISOString().slice(0, 10),
        ),
        isLoading: false,
      };
    }
    return { items: mocks.items, isLoading: false };
  },
  useAllOpenActionItemsCount: () => mocks.openCount,
}));

vi.mock("~/store/zustand/tabs", () => ({
  useTabs: (
    selector: (state: { openCurrent: typeof mocks.openCurrent }) => unknown,
  ) => selector({ openCurrent: mocks.openCurrent }),
}));

import { TabContentActionItems } from "./index";

describe("TabContentActionItems", () => {
  beforeEach(() => {
    mocks.items = [];
    mocks.doneItems = [];
    mocks.openCount = 0;
    mocks.openCurrent.mockClear();
    mocks.setActionItemStatus.mockClear();
  });

  afterEach(() => {
    cleanup();
  });

  it("renders empty state when there are no action items", () => {
    render(<TabContentActionItems />);
    expect(screen.getByText("Action items")).toBeDefined();
    expect(screen.getByText("0 open")).toBeDefined();
    expect(screen.getByText("No action items.")).toBeDefined();
  });

  it("renders action items grouped by meeting with metadata and badges", () => {
    mocks.openCount = 2;
    mocks.items = [
      {
        id: "ai-1",
        text: "Finalize gateway auth rotation",
        status: "todo",
        due_at: "2026-11-10",
        assignee_human_id: "human-1",
        owner_name: "Alice Sharma",
        session_id: "sess-1",
        session_title: "Q4 Launch Sync",
        session_date: "2026-11-04T10:00:00Z",
        series_id: "series-1",
      },
      {
        id: "ai-2",
        text: "Publish rollout checklist",
        status: "todo",
        due_at: "2026-11-01",
        assignee_human_id: "user-1",
        owner_name: "Abhishek Gupta",
        session_id: "sess-2",
        session_title: "Sprint Planning",
        session_date: "2026-11-03T09:00:00Z",
        series_id: "",
        carried_from_session_id: "sess-prev",
        carried_from_session_title: "Sprint Planning Prev",
        carried_from_session_date: "2026-10-28T09:00:00Z",
      },
    ];

    render(<TabContentActionItems />);

    expect(screen.getByText("2 open")).toBeDefined();
    expect(screen.getByText("Q4 Launch Sync")).toBeDefined();
    expect(screen.getByText("Sprint Planning")).toBeDefined();
    expect(screen.getByText("Finalize gateway auth rotation")).toBeDefined();
    expect(screen.getByText("Publish rollout checklist")).toBeDefined();
    expect(screen.getByText("Carried over")).toBeDefined();
    expect(
      screen.getByText("from Oct 28 · Sprint Planning Prev"),
    ).toBeDefined();
    expect(screen.getByText("Alice Sharma")).toBeDefined();
    expect(screen.getByText("Abhishek Gupta")).toBeDefined();
  });

  it("toggles action item status when checkbox is clicked", () => {
    mocks.openCount = 1;
    mocks.items = [
      {
        id: "ai-1",
        text: "Test Tauri build",
        status: "todo",
        due_at: "",
        assignee_human_id: "",
        owner_name: "",
        session_id: "sess-1",
        session_title: "Review",
        session_date: "2026-11-04T10:00:00Z",
        series_id: "",
      },
    ];

    render(<TabContentActionItems />);

    const checkbox = screen.getByRole("checkbox", {
      name: "Test Tauri build",
    });
    fireEvent.click(checkbox);

    expect(mocks.setActionItemStatus).toHaveBeenCalledWith(
      "ai-1",
      "done",
      "user-1",
    );
  });

  it("switches filter when filter pills are clicked", () => {
    mocks.openCount = 2;
    mocks.items = [
      {
        id: "ai-1",
        text: "Mine only item",
        status: "todo",
        due_at: "2026-11-10",
        assignee_human_id: "user-1",
        owner_name: "Abhishek Gupta",
        session_id: "sess-1",
        session_title: "Sync",
        session_date: "2026-11-04T10:00:00Z",
        series_id: "",
      },
      {
        id: "ai-2",
        text: "Other person item",
        status: "todo",
        due_at: "2026-11-10",
        assignee_human_id: "other",
        owner_name: "Bob",
        session_id: "sess-1",
        session_title: "Sync",
        session_date: "2026-11-04T10:00:00Z",
        series_id: "",
      },
    ];

    render(<TabContentActionItems />);
    expect(screen.getByText("Mine only item")).toBeDefined();
    expect(screen.getByText("Other person item")).toBeDefined();

    const minePill = screen.getByRole("button", { name: "Mine" });
    fireEvent.click(minePill);

    expect(screen.getByText("Mine only item")).toBeDefined();
    expect(screen.queryByText("Other person item")).toBeNull();
  });

  it("renders done disclosure when completed items exist", () => {
    mocks.openCount = 0;
    mocks.items = [];
    mocks.doneItems = [
      {
        id: "ai-done-1",
        text: "Shipped release notes",
        status: "done",
        due_at: "",
        assignee_human_id: "",
        owner_name: "Alice",
        session_id: "sess-1",
        session_title: "Retro",
        session_date: "2026-11-04T10:00:00Z",
        series_id: "",
      },
    ];

    render(<TabContentActionItems />);

    expect(screen.getByText("Done (1)")).toBeDefined();
    expect(screen.getByText("Shipped release notes")).toBeDefined();
  });

  it("navigates to session when meeting heading is clicked", () => {
    mocks.items = [
      {
        id: "ai-1",
        text: "Follow up",
        status: "todo",
        due_at: "",
        assignee_human_id: "",
        owner_name: "",
        session_id: "sess-xyz",
        session_title: "Design Critique",
        session_date: "2026-11-04T10:00:00Z",
        series_id: "",
      },
    ];

    render(<TabContentActionItems />);

    const meetingButton = screen.getByRole("button", {
      name: "Design Critique",
    });
    fireEvent.click(meetingButton);

    expect(mocks.openCurrent).toHaveBeenCalledWith({
      type: "sessions",
      id: "sess-xyz",
    });
  });
});

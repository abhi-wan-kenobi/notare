import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  currentTab: { type: "empty" } as { type: string; id?: string } | null,
  recentlyOpenedSessionIds: [] as string[],
  openCurrent: vi.fn(),
  openNew: vi.fn(),
  select: vi.fn(),
  openDialog: vi.fn(),
  liveQueryCount: 0,
}));

vi.mock("~/store/zustand/tabs", () => {
  const state = {
    get currentTab() {
      return mocks.currentTab;
    },
    get recentlyOpenedSessionIds() {
      return mocks.recentlyOpenedSessionIds;
    },
    openCurrent: mocks.openCurrent,
    openNew: mocks.openNew,
    select: mocks.select,
    tabs: [] as unknown[],
  };
  const useTabs = (selector: (s: typeof state) => unknown) => selector(state);
  useTabs.getState = () => state;
  return { useTabs };
});

vi.mock("~/db", () => ({
  useLiveQuery: () => ({ data: mocks.liveQueryCount }),
}));

vi.mock("~/session/queries", () => ({
  useSessionSummaries: () => [
    { id: "s1", title: "Q4 Launch Sync", created_at: "2026-11-04T10:00:00Z" },
    { id: "s2", title: "Sprint Planning", created_at: "2026-11-03T09:00:00Z" },
  ],
}));

vi.mock("~/shared/open-note-dialog", () => ({
  useOpenNoteDialog: () => ({ open: mocks.openDialog }),
}));

vi.mock("@tauri-apps/plugin-os", () => ({
  platform: () => "macos",
}));

vi.mock("~/sidebar/calendar", () => ({
  CalendarNav: () => <div data-testid="calendar-nav" />,
}));

vi.mock("~/sidebar/contacts", () => ({
  ContactsNav: () => <div data-testid="contacts-nav" />,
}));

vi.mock("~/sidebar/settings", () => ({
  SettingsNav: () => <div data-testid="settings-nav" />,
}));

vi.mock("~/sidebar/templates", () => ({
  TemplatesNav: () => <div data-testid="templates-nav" />,
}));

import { LeftSidebar } from "./index";

describe("LeftSidebar", () => {
  beforeEach(() => {
    mocks.currentTab = { type: "empty" };
    mocks.recentlyOpenedSessionIds = [];
    mocks.openCurrent.mockClear();
    mocks.openNew.mockClear();
    mocks.select.mockClear();
    mocks.openDialog.mockClear();
    mocks.liveQueryCount = 0;
    localStorage.clear();
  });

  afterEach(() => {
    cleanup();
  });

  it("renders the default sidebar content without a duplicate top offset", () => {
    const { container } = render(<LeftSidebar />);

    expect(screen.getByTestId("sidebar-default-content")).toBeTruthy();
    expect(container.firstElementChild?.className).toContain("pt-0");
    expect(container.firstElementChild?.className).not.toContain("pr-1");
  });

  it("renders timelineHeader above the default content in the timeline layout", () => {
    render(
      <LeftSidebar timelineHeader={<div data-testid="timeline-header" />} />,
    );

    expect(
      screen
        .getByTestId("timeline-header")
        .parentElement?.contains(screen.getByTestId("sidebar-default-content")),
    ).toBe(true);
  });

  it.each([
    ["settings", "settings-nav"],
    ["calendar", "calendar-nav"],
    ["contacts", "contacts-nav"],
    ["templates", "templates-nav"],
  ])("keeps %s below the window chrome", (type, testId) => {
    mocks.currentTab = { type };

    const { container } = render(<LeftSidebar />);
    const classList = container.firstElementChild?.className.split(" ") ?? [];

    expect(screen.getByTestId(testId)).toBeTruthy();
    expect(classList).toContain("pt-11");
    expect(classList).toContain("pr-1");
    expect(classList).not.toContain("pt-0");
  });

  it("shows the wordmark, search field, and primary nav rows", () => {
    render(<LeftSidebar />);

    expect(screen.getByText("Notare")).toBeTruthy();
    expect(screen.getByText("Search notes and actions")).toBeTruthy();
    expect(screen.getByRole("button", { name: /Home/ })).toBeTruthy();
    expect(screen.getByRole("button", { name: /Meetings/ })).toBeTruthy();
    expect(screen.getByRole("button", { name: /Calendar/ })).toBeTruthy();
    expect(screen.getByRole("button", { name: /Action items/ })).toBeTruthy();
    expect(screen.getByRole("button", { name: /Settings/ })).toBeTruthy();
  });

  it("opens the command palette from the search field", () => {
    render(<LeftSidebar />);

    fireEvent.click(screen.getByText("Search notes and actions"));

    expect(mocks.openDialog).toHaveBeenCalled();
  });

  it("marks home active on the empty tab and meetings active on a session", () => {
    mocks.currentTab = { type: "empty" };
    const { rerender } = render(<LeftSidebar />);
    expect(
      screen.getByRole("button", { name: /Home/ }).getAttribute("aria-current"),
    ).toBe("page");

    mocks.currentTab = { type: "sessions", id: "s1" };
    rerender(<LeftSidebar />);
    expect(
      screen
        .getByRole("button", { name: /Meetings/ })
        .getAttribute("aria-current"),
    ).toBe("page");
  });

  it("shows the open action items count as a badge", () => {
    mocks.liveQueryCount = 3;
    render(<LeftSidebar />);

    expect(
      screen.getByRole("button", { name: /Action items/ }).textContent,
    ).toContain("3");
  });

  it("opens each primary and library surface", () => {
    render(<LeftSidebar />);

    fireEvent.click(screen.getByRole("button", { name: /Meetings/ }));
    expect(mocks.openNew).toHaveBeenCalledWith({ type: "meetings" });

    fireEvent.click(screen.getByRole("button", { name: /Calendar/ }));
    expect(mocks.openNew).toHaveBeenCalledWith({ type: "calendar" });

    fireEvent.click(screen.getByRole("button", { name: /Action items/ }));
    expect(mocks.openNew).toHaveBeenCalledWith({ type: "action-items" });

    fireEvent.click(screen.getByRole("button", { name: "Library" }));
    fireEvent.click(screen.getByRole("button", { name: /Templates/ }));
    expect(mocks.openNew).toHaveBeenCalledWith({ type: "templates" });

    fireEvent.click(screen.getByRole("button", { name: /Settings/ }));
    expect(mocks.openNew).toHaveBeenCalledWith({ type: "settings" });
  });

  it("shows the platform settings shortcut hint", () => {
    render(<LeftSidebar />);

    expect(
      screen.getByRole("button", { name: /Settings/ }).textContent,
    ).toContain("⌘ ,");
  });

  it("collapses the library by default and expands it on click", () => {
    render(<LeftSidebar />);

    expect(screen.queryByRole("button", { name: /Templates/ })).toBeNull();

    fireEvent.click(screen.getByRole("button", { name: "Library" }));

    expect(screen.getByRole("button", { name: /Templates/ })).toBeTruthy();
    expect(screen.getByRole("button", { name: /Snippets/ })).toBeTruthy();
    expect(screen.getByRole("button", { name: /Contacts/ })).toBeTruthy();
    expect(screen.getByRole("button", { name: /Trash/ })).toBeTruthy();
  });

  it("lists up to 5 recently-opened sessions and opens one on click", () => {
    mocks.recentlyOpenedSessionIds = ["s1", "s2"];
    render(<LeftSidebar />);

    expect(screen.getByText("Q4 Launch Sync")).toBeTruthy();
    fireEvent.click(screen.getByText("Q4 Launch Sync"));

    expect(mocks.openCurrent).toHaveBeenCalledWith({
      type: "sessions",
      id: "s1",
    });
  });
});

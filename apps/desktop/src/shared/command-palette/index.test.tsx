import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  openCurrent: vi.fn(),
  openNew: vi.fn(),
  toggleInspector: vi.fn(),
  openWith: vi.fn(),
  createNote: vi.fn(),
  startRecording: vi.fn(),
  setTheme: vi.fn(),
  openSurfaceTab: vi.fn(),
  sessions: [] as {
    id: string;
    title: string;
    created_at: string;
  }[],
}));

const tabsState = {
  openCurrent: mocks.openCurrent,
  openNew: mocks.openNew,
  recentlyOpenedSessionIds: ["recent-session"],
};

vi.mock("@tauri-apps/plugin-os", () => ({ platform: () => "macos" }));
vi.mock("react-hotkeys-hook", () => ({ useHotkeys: vi.fn() }));
vi.mock("~/db", () => ({
  useLiveQuery: () => ({ data: [] }),
}));
vi.mock("~/calendar/hooks", () => ({
  toTz: (date: Date | string) => new Date(date),
  useNow: () => new Date("2026-11-05T12:00:00.000Z"),
  useTimezone: () => undefined,
}));
vi.mock("~/calendar/queries", () => ({
  useTimelineEventsTable: () => ({
    eventToday: {
      title: "Design review",
      started_at: "2026-11-05T15:30:00.000Z",
    },
  }),
}));
vi.mock("~/chat/recipes", () => ({
  useRecipes: () => ({
    recipes: [
      {
        id: "recipe-1",
        title: "Summarize for my manager",
        prompt: "Write a concise manager summary",
      },
    ],
  }),
}));
vi.mock("~/contexts/shell", () => ({
  useShell: () => ({
    inspector: { toggle: mocks.toggleInspector, openWith: mocks.openWith },
  }),
}));
vi.mock("~/session/queries", () => ({
  useSessionSummaries: () => mocks.sessions,
}));
vi.mock("~/shared/config", () => ({ useConfigValue: () => "dark" }));
vi.mock("~/shared/main/content-offset", () => ({
  useMainContentCenterOffset: () => 0,
}));
vi.mock("~/shared/useNewNote", () => ({
  useNewNote: () => mocks.createNote,
  useNewNoteAndListen: () => mocks.startRecording,
}));
vi.mock("~/shared/theme/apply", () => ({
  applyDocumentTheme: vi.fn(),
  writeStoredThemePreference: vi.fn(),
}));
vi.mock("~/settings/queries", () => ({
  useSetSettingValue: () => mocks.setTheme,
}));
vi.mock("~/sidebar/surface-navigation", () => ({
  openSurfaceTab: mocks.openSurfaceTab,
}));
vi.mock("~/store/zustand/tabs", () => ({
  useTabs: (selector: (state: typeof tabsState) => unknown) =>
    selector(tabsState),
}));

import { OpenNoteDialog } from "./index";

const renderPalette = () =>
  render(<OpenNoteDialog open onOpenChange={vi.fn()} />);

describe("command palette", () => {
  beforeEach(() => {
    vi.stubGlobal(
      "ResizeObserver",
      class {
        observe() {}
        unobserve() {}
        disconnect() {}
      },
    );
    Object.defineProperty(HTMLElement.prototype, "scrollIntoView", {
      configurable: true,
      value: vi.fn(),
    });
    vi.clearAllMocks();
    mocks.sessions = [
      {
        id: "recent-session",
        title: "Q4 launch sync",
        created_at: "2026-11-04T10:00:00.000Z",
      },
      {
        id: "older-session",
        title: "Sprint planning",
        created_at: "2026-11-03T09:00:00.000Z",
      },
    ];
  });
  afterEach(() => {
    cleanup();
    vi.unstubAllGlobals();
    Reflect.deleteProperty(HTMLElement.prototype, "scrollIntoView");
  });

  it("opens a matching meeting from a search query", () => {
    renderPalette();
    fireEvent.change(
      screen.getByPlaceholderText("Search notes, pages, and actions…"),
      { target: { value: "Sprint planning" } },
    );

    fireEvent.click(screen.getByText("Sprint planning"));

    expect(mocks.openCurrent).toHaveBeenCalledWith({
      type: "sessions",
      id: "older-session",
    });
  });

  it("runs a selected recipe in the inspector with its stored prompt", () => {
    renderPalette();
    fireEvent.click(screen.getByText("Summarize for my manager"));

    expect(mocks.openWith).toHaveBeenCalledWith(
      "Write a concise manager summary",
    );
  });

  it("triggers actions for new note and start recording", () => {
    renderPalette();
    fireEvent.click(screen.getByText("New note"));
    expect(mocks.createNote).toHaveBeenCalledTimes(1);

    fireEvent.click(screen.getByText("Start recording"));
    expect(mocks.startRecording).toHaveBeenCalledTimes(1);
  });

  it("navigates to primary surfaces", () => {
    renderPalette();
    fireEvent.click(screen.getByText("Open Home"));
    expect(mocks.openSurfaceTab).toHaveBeenCalledWith("home");

    fireEvent.click(screen.getByText("Open Meetings"));
    expect(mocks.openSurfaceTab).toHaveBeenCalledWith("meetings");

    fireEvent.click(screen.getByText("Open Calendar"));
    expect(mocks.openSurfaceTab).toHaveBeenCalledWith("calendar");

    fireEvent.click(screen.getByText("Open Action items"));
    expect(mocks.openSurfaceTab).toHaveBeenCalledWith("action-items");
  });

  it("toggles inspector and theme", () => {
    renderPalette();
    fireEvent.click(screen.getByText("Toggle inspector"));
    expect(mocks.toggleInspector).toHaveBeenCalledTimes(1);

    fireEvent.click(screen.getByText("Switch to light theme"));
    expect(mocks.setTheme).toHaveBeenCalledWith("light");
  });

  it("opens settings hubs", () => {
    renderPalette();
    fireEvent.click(screen.getByText("Open settings · General & System"));
    expect(mocks.openNew).toHaveBeenCalledWith({
      type: "settings",
      state: { tab: "general" },
    });

    fireEvent.click(screen.getByText("Open settings · AI & Speech"));
    expect(mocks.openNew).toHaveBeenCalledWith({
      type: "settings",
      state: { tab: "ai" },
    });

    fireEvent.click(screen.getByText("Open settings · Integrations & Sync"));
    expect(mocks.openNew).toHaveBeenCalledWith({
      type: "settings",
      state: { tab: "integrations" },
    });

    fireEvent.click(screen.getByText("Open settings · Account & Storage"));
    expect(mocks.openNew).toHaveBeenCalledWith({
      type: "settings",
      state: { tab: "account" },
    });
  });

  it("includes and opens today's calendar events without any meetings", () => {
    mocks.sessions = [];
    renderPalette();

    fireEvent.click(screen.getByText("Design review"));

    expect(mocks.openSurfaceTab).toHaveBeenCalledWith("calendar");
  });
});

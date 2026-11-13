import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

// Everything the real providers reach for is stubbed; `~/contexts/shell` and
// `~/shared/command-palette` stay real, because the bug this guards against
// (0.7.0: the palette rendered outside ShellProvider and the whole main window
// fell over to the error boundary) lives in how those two nest.
const passthrough = vi.hoisted(
  () =>
    ({ children }: { children: React.ReactNode }) =>
      children,
);

vi.mock("@tanstack/react-router", () => ({
  useRouteContext: () => ({ aiTaskStore: {}, toolRegistry: {} }),
}));
vi.mock("@hypr/editor/task-storage", () => ({
  TaskStorageProvider: passthrough,
}));
vi.mock("./lifecycle", () => ({ ClassicMainServices: () => null }));
vi.mock("~/ai/contexts", () => ({ AITaskProvider: passthrough }));
vi.mock("~/contexts/notifications", () => ({
  NotificationProvider: passthrough,
}));
vi.mock("~/contexts/tool", () => ({ ToolRegistryProvider: passthrough }));
vi.mock("~/editor-bridge/task-storage", () => ({
  useStoreBackedTaskStorage: () => ({}),
}));
vi.mock("~/search/contexts/engine", () => ({
  SearchEngineProvider: passthrough,
}));

vi.mock("~/chat/state/use-chat-mode", () => ({ useChatMode: () => ({}) }));
vi.mock("~/contexts/shell/inspector", () => ({
  useInspector: () => ({ toggle: vi.fn(), openWith: vi.fn() }),
}));
vi.mock("~/contexts/shell/leftsidebar", () => ({ useLeftSidebar: () => ({}) }));
vi.mock("~/contexts/shell/settings", () => ({ useSettings: () => ({}) }));

vi.mock("@tauri-apps/plugin-os", () => ({ platform: () => "macos" }));
vi.mock("react-hotkeys-hook", () => ({ useHotkeys: vi.fn() }));
vi.mock("~/db", () => ({ useLiveQuery: () => ({ data: [] }) }));
vi.mock("~/calendar/hooks", () => ({
  toTz: (date: Date | string) => new Date(date),
  useNow: () => new Date("2026-11-05T12:00:00.000Z"),
  useTimezone: () => undefined,
}));
vi.mock("~/calendar/queries", () => ({
  useTimelineEventsTable: () => ({}),
}));
vi.mock("~/chat/recipes", () => ({ useRecipes: () => ({ recipes: [] }) }));
vi.mock("~/session/queries", () => ({ useSessionSummaries: () => [] }));
vi.mock("~/shared/config", () => ({ useConfigValue: () => "light" }));
vi.mock("~/shared/main/content-offset", () => ({
  useMainContentCenterOffset: () => 0,
}));
vi.mock("~/shared/useNewNote", () => ({
  useNewNote: () => vi.fn(),
  useNewNoteAndListen: () => vi.fn(),
}));
vi.mock("~/shared/theme/apply", () => ({
  applyDocumentTheme: vi.fn(),
  writeStoredThemePreference: vi.fn(),
}));
vi.mock("~/settings/queries", () => ({ useSetSettingValue: () => vi.fn() }));
vi.mock("~/sidebar/surface-navigation", () => ({ openSurfaceTab: vi.fn() }));
vi.mock("~/store/zustand/tabs", () => ({
  useTabs: (selector: (state: unknown) => unknown) =>
    selector({
      openCurrent: vi.fn(),
      openNew: vi.fn(),
      recentlyOpenedSessionIds: [],
    }),
}));

import { ClassicMainLayout } from "./layout";

describe("ClassicMainLayout", () => {
  afterEach(cleanup);

  it("mounts the main window with the command palette inside the shell", () => {
    render(
      <ClassicMainLayout>
        <div>main window</div>
      </ClassicMainLayout>,
    );

    expect(screen.getByText("main window")).toBeTruthy();
  });
});

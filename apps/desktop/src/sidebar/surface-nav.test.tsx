import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

type MockTab = {
  type: string;
  id?: string;
  active?: boolean;
  slotId: string;
  returnToSlotId?: string;
  returnToTabId?: string;
};

const mocks = vi.hoisted(() => ({
  state: {
    currentTab: null as {
      type: string;
      id?: string;
      active?: boolean;
      slotId: string;
      returnToSlotId?: string;
      returnToTabId?: string;
    } | null,
    tabs: [] as {
      type: string;
      id?: string;
      active?: boolean;
      slotId: string;
      returnToSlotId?: string;
      returnToTabId?: string;
    }[],
    select: vi.fn(),
    openNew: vi.fn(),
    openCurrent: vi.fn(),
  },
  uniqueIdfromTab: (tab: { type: string; id?: string }) =>
    tab.id ? `${tab.type}-${tab.id}` : tab.type,
}));

const lingui = vi.hoisted(() => {
  const t = (
    input: TemplateStringsArray | { message?: string } | string,
    ...values: unknown[]
  ) => {
    if (Array.isArray(input)) {
      return input.reduce(
        (message, part, index) =>
          `${message}${part}${index < values.length ? String(values[index]) : ""}`,
        "",
      );
    }

    if (typeof input === "string") {
      return input;
    }

    if ("message" in input) {
      return input.message ?? "";
    }

    return "";
  };

  return { t };
});

vi.mock("@lingui/react/macro", () => ({
  Trans: ({
    children,
    id,
    message,
  }: {
    children?: ReactNode;
    id?: string;
    message?: string;
  }) => <>{children ?? message ?? id}</>,
  useLingui: () => ({
    _: lingui.t,
    t: lingui.t,
  }),
}));

vi.mock("@tauri-apps/plugin-os", () => ({
  platform: () => "windows",
}));

vi.mock("@hypr/ui/components/ui/tooltip", () => ({
  Tooltip: ({ children }: { children: ReactNode }) => <>{children}</>,
  TooltipTrigger: ({ children }: { children: ReactNode }) => <>{children}</>,
  TooltipContent: ({ children }: { children: ReactNode }) => (
    <span data-testid="tooltip-content">{children}</span>
  ),
}));

vi.mock("~/store/zustand/tabs", () => {
  const useTabs = (selector: (state: typeof mocks.state) => unknown) =>
    selector(mocks.state);
  useTabs.getState = () => mocks.state;

  return {
    useTabs,
    uniqueIdfromTab: mocks.uniqueIdfromTab,
  };
});

import { SidebarSurfaceNav } from "./surface-nav";

describe("SidebarSurfaceNav", () => {
  beforeEach(() => {
    mocks.state.currentTab = {
      type: "sessions",
      id: "note-1",
      slotId: "slot-1",
    };
    mocks.state.tabs = [mocks.state.currentTab];
    mocks.state.select.mockClear();
    mocks.state.openNew.mockClear();
    mocks.state.openCurrent.mockClear();
  });

  afterEach(() => {
    cleanup();
  });

  it("renders one button per major surface", () => {
    render(<SidebarSurfaceNav />);

    for (const surface of [
      "notes",
      "calendar",
      "contacts",
      "templates",
      "settings",
    ]) {
      expect(
        screen.getByTestId(`sidebar-surface-nav-${surface}`),
      ).toBeTruthy();
    }
  });

  it("marks notes active while a note is open", () => {
    render(<SidebarSurfaceNav />);

    expect(
      screen
        .getByTestId("sidebar-surface-nav-notes")
        .getAttribute("aria-current"),
    ).toBe("page");
    expect(
      screen
        .getByTestId("sidebar-surface-nav-settings")
        .getAttribute("aria-current"),
    ).toBeNull();
  });

  it("marks settings active while settings is open", () => {
    mocks.state.currentTab = { type: "settings", slotId: "slot-2" };
    mocks.state.tabs = [mocks.state.currentTab];

    render(<SidebarSurfaceNav />);

    expect(
      screen
        .getByTestId("sidebar-surface-nav-settings")
        .getAttribute("aria-current"),
    ).toBe("page");
  });

  it("opens settings from a note with one click", () => {
    render(<SidebarSurfaceNav />);

    fireEvent.click(screen.getByTestId("sidebar-surface-nav-settings"));

    expect(mocks.state.openNew).toHaveBeenCalledWith({ type: "settings" });
  });

  it("opens calendar, contacts and templates surfaces", () => {
    render(<SidebarSurfaceNav />);

    fireEvent.click(screen.getByTestId("sidebar-surface-nav-calendar"));
    fireEvent.click(screen.getByTestId("sidebar-surface-nav-contacts"));
    fireEvent.click(screen.getByTestId("sidebar-surface-nav-templates"));

    expect(mocks.state.openNew).toHaveBeenCalledWith({ type: "calendar" });
    expect(mocks.state.openNew).toHaveBeenCalledWith({
      type: "contacts",
      state: { selected: null },
    });
    expect(mocks.state.openNew).toHaveBeenCalledWith({ type: "templates" });
  });

  it("returns to the origin note when leaving settings via notes", () => {
    const noteTab: MockTab = {
      type: "sessions",
      id: "note-1",
      slotId: "slot-1",
    };
    mocks.state.currentTab = {
      type: "settings",
      slotId: "slot-2",
      returnToSlotId: "slot-1",
      returnToTabId: "sessions-note-1",
    };
    mocks.state.tabs = [noteTab, mocks.state.currentTab];

    render(<SidebarSurfaceNav />);

    fireEvent.click(screen.getByTestId("sidebar-surface-nav-notes"));

    expect(mocks.state.select).toHaveBeenCalledWith(noteTab);
    expect(mocks.state.openCurrent).not.toHaveBeenCalled();
  });

  it("falls back to the existing home tab when the origin is gone", () => {
    const homeTab: MockTab = { type: "empty", slotId: "slot-3" };
    mocks.state.currentTab = {
      type: "settings",
      slotId: "slot-2",
      returnToSlotId: "slot-1",
    };
    mocks.state.tabs = [homeTab, mocks.state.currentTab];

    render(<SidebarSurfaceNav />);

    fireEvent.click(screen.getByTestId("sidebar-surface-nav-notes"));

    expect(mocks.state.select).toHaveBeenCalledWith(homeTab);
  });

  it("opens home when leaving settings with no origin and no home tab", () => {
    mocks.state.currentTab = { type: "settings", slotId: "slot-2" };
    mocks.state.tabs = [mocks.state.currentTab];

    render(<SidebarSurfaceNav />);

    fireEvent.click(screen.getByTestId("sidebar-surface-nav-notes"));

    expect(mocks.state.openCurrent).toHaveBeenCalledWith({ type: "empty" });
  });

  it("does nothing when notes is clicked on a notes surface", () => {
    render(<SidebarSurfaceNav />);

    fireEvent.click(screen.getByTestId("sidebar-surface-nav-notes"));

    expect(mocks.state.select).not.toHaveBeenCalled();
    expect(mocks.state.openNew).not.toHaveBeenCalled();
    expect(mocks.state.openCurrent).not.toHaveBeenCalled();
  });

  it("shows the settings shortcut hint for the current platform", () => {
    render(<SidebarSurfaceNav />);

    const contents = screen
      .getAllByTestId("tooltip-content")
      .map((node) => node.textContent);

    expect(contents).toContain("Settings (Ctrl+,)");
  });
});

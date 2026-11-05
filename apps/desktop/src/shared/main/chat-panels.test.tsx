import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  inspectorOpen: false,
  currentTab: { type: "empty" } as { type: string } | null,
  leftSidebarExpanded: true,
  sendEvent: vi.fn(),
  sessionProps: { sessionId: "chat-session-1" },
  setLeftSidebarExpanded: vi.fn(),
  windowExpandWidth: vi.fn(() => Promise.resolve({ status: "ok", data: null })),
  windowRestoreWidth: vi.fn(() =>
    Promise.resolve({ status: "ok", data: null }),
  ),
}));

vi.mock("@tauri-apps/api/core", () => ({
  isTauri: () => true,
}));

vi.mock("@hypr/plugin-windows", () => ({
  commands: {
    windowExpandWidth: mocks.windowExpandWidth,
    windowRestoreWidth: mocks.windowRestoreWidth,
  },
}));

vi.mock("~/contexts/shell", () => ({
  useShell: () => ({
    inspector: {
      open: mocks.inspectorOpen,
    },
    leftsidebar: {
      expanded: mocks.leftSidebarExpanded,
      setExpanded: mocks.setLeftSidebarExpanded,
    },
  }),
}));

vi.mock("~/store/zustand/tabs", () => ({
  useTabs: (
    selector: (state: { currentTab: typeof mocks.currentTab }) => unknown,
  ) => selector({ currentTab: mocks.currentTab }),
}));

vi.mock("~/chat/components/chat-panel", () => ({
  ChatPanelFrame: ({ sessionProps }: { sessionProps: unknown }) => (
    <div
      data-has-session={String(sessionProps === mocks.sessionProps)}
      data-testid="chat-view"
    >
      Chat
    </div>
  ),
  ChatSessionHost: ({
    children,
  }: {
    children: (sessionProps: unknown) => React.ReactNode;
  }) => <>{children(mocks.sessionProps)}</>,
}));

import { MainChatPanels } from "./chat-panels";

let restorePanelWidths: (() => void) | null = null;

describe("MainChatPanels", () => {
  beforeEach(() => {
    cleanup();
    restorePanelWidths?.();
    restorePanelWidths = null;
    mocks.inspectorOpen = false;
    mocks.currentTab = { type: "empty" };
    mocks.leftSidebarExpanded = true;
    mocks.sendEvent.mockClear();
    mocks.setLeftSidebarExpanded.mockClear();
    mocks.windowExpandWidth.mockClear();
    mocks.windowRestoreWidth.mockClear();
  });

  it("renders the main content with the inspector closed by default", () => {
    render(
      <MainChatPanels>
        <div data-testid="main-content" />
      </MainChatPanels>,
    );

    expect(screen.getByTestId("main-content")).toBeTruthy();
    const rightPanel = document.querySelector("[data-chat-right-panel]");
    expect(rightPanel).toBeInstanceOf(HTMLDivElement);
    expect(rightPanel?.getAttribute("aria-hidden")).toBe("true");
    expect((rightPanel as HTMLElement).style.width).toBe("0px");
    expect(rightPanel?.className).not.toContain("border-l ");
    expect(rightPanel?.className).toContain("border-l-0");
  });

  it("renders the inspector at a fixed 360px when open", () => {
    mocks.inspectorOpen = true;

    render(
      <MainChatPanels>
        <div data-testid="main-content" />
      </MainChatPanels>,
    );

    const rightPanel = document.querySelector("[data-chat-right-panel]");

    expect(rightPanel?.getAttribute("aria-hidden")).toBe("false");
    expect((rightPanel as HTMLElement).style.width).toBe("360px");
    expect(rightPanel?.className).toContain("border-l");
    expect(rightPanel?.className).not.toContain("border-l-0");
    expect(rightPanel?.className).toContain("bg-card");
    expect(screen.getByTestId("chat-view").dataset.hasSession).toBe("true");
  });

  it("reserves enough main-body width for a 500px note surface beside the sidebar", () => {
    mocks.currentTab = { type: "sessions" };
    mocks.leftSidebarExpanded = true;

    render(
      <MainChatPanels>
        <div data-testid="main-content" />
      </MainChatPanels>,
    );

    const bodyPanel = document.querySelector(
      "[data-main-body-panel-container]",
    ) as HTMLElement;
    expect(bodyPanel.style.minWidth).toBe("700px");
  });

  it("reserves enough main-body width for the empty surface beside the sidebar", () => {
    mocks.currentTab = { type: "empty" };
    mocks.leftSidebarExpanded = true;

    render(
      <MainChatPanels>
        <div data-testid="main-content" />
      </MainChatPanels>,
    );

    const bodyPanel = document.querySelector(
      "[data-main-body-panel-container]",
    ) as HTMLElement;
    expect(bodyPanel.style.minWidth).toBe("700px");
  });

  it("expands left when opening the sidebar would make a note surface narrower than 500px", () => {
    mocks.currentTab = { type: "sessions" };
    mocks.leftSidebarExpanded = true;
    mockPanelWidths({
      bodyPanelWidth: 640,
      leftSidebarWidth: 200,
    });

    render(
      <MainChatPanels>
        <div data-left-sidebar-chrome />
        <div data-chat-floating-anchor>
          <div data-session-surface />
        </div>
      </MainChatPanels>,
    );

    expect(mocks.windowExpandWidth).toHaveBeenCalledWith(
      60,
      null,
      false,
      true,
      false,
    );
  });

  it("expands left when opening the sidebar would make the empty surface narrower than 500px", () => {
    mocks.currentTab = { type: "empty" };
    mocks.leftSidebarExpanded = true;
    mockPanelWidths({
      bodyPanelWidth: 640,
      leftSidebarWidth: 200,
    });

    render(
      <MainChatPanels>
        <div data-left-sidebar-chrome />
        <div data-chat-floating-anchor>
          <div data-testid="empty-surface" />
        </div>
      </MainChatPanels>,
    );

    expect(mocks.windowExpandWidth).toHaveBeenCalledWith(
      60,
      null,
      false,
      true,
      false,
    );
  });

  it("expands right when docked chat would make a note surface narrower than 500px", () => {
    mocks.inspectorOpen = true;
    mocks.currentTab = { type: "sessions" };
    mocks.leftSidebarExpanded = false;
    mockPanelWidths({
      bodyPanelWidth: 460,
      leftSidebarWidth: 0,
      rightPanelWidth: 120,
    });

    render(
      <MainChatPanels>
        <div data-chat-floating-anchor>
          <div data-session-surface />
        </div>
      </MainChatPanels>,
    );

    expect(mocks.windowExpandWidth).toHaveBeenCalledWith(
      280,
      null,
      false,
      false,
      true,
    );
  });

  it("collapses the left sidebar when docked chat would make the note surface narrower than 500px", () => {
    mocks.inspectorOpen = true;
    mocks.currentTab = { type: "sessions" };
    mocks.leftSidebarExpanded = true;
    mockPanelWidths({
      bodyPanelWidth: 650,
      leftSidebarWidth: 200,
      rightPanelWidth: 360,
    });

    render(
      <MainChatPanels>
        <div data-left-sidebar-chrome />
        <div data-chat-floating-anchor>
          <div data-session-surface />
        </div>
      </MainChatPanels>,
    );

    expect(mocks.setLeftSidebarExpanded).toHaveBeenCalledWith(false);
    expect(mocks.windowExpandWidth).not.toHaveBeenCalled();
  });

  it("expands right when docked chat renders narrower than 360px", () => {
    mocks.inspectorOpen = true;
    mocks.currentTab = { type: "sessions" };
    mocks.leftSidebarExpanded = true;
    mockPanelWidths({
      bodyPanelWidth: 700,
      leftSidebarWidth: 200,
      rightPanelWidth: 120,
    });

    render(
      <MainChatPanels>
        <div data-left-sidebar-chrome />
        <div data-chat-floating-anchor>
          <div data-session-surface />
        </div>
      </MainChatPanels>,
    );

    expect(mocks.windowExpandWidth).toHaveBeenCalledWith(
      240,
      null,
      false,
      false,
      true,
    );
  });

  it("does not restore window width after closing a left-sidebar expansion", () => {
    mocks.currentTab = { type: "sessions" };
    mocks.leftSidebarExpanded = true;
    mockPanelWidths({
      bodyPanelWidth: 640,
      leftSidebarWidth: 200,
    });

    const renderPanels = () => (
      <MainChatPanels>
        <div data-left-sidebar-chrome />
        <div data-chat-floating-anchor>
          <div data-session-surface />
        </div>
      </MainChatPanels>
    );
    const { rerender } = render(renderPanels());

    expect(mocks.windowExpandWidth).toHaveBeenCalledWith(
      60,
      null,
      false,
      true,
      false,
    );

    mocks.leftSidebarExpanded = false;
    rerender(renderPanels());

    expect(mocks.windowRestoreWidth).not.toHaveBeenCalled();
  });

  it("restores window width expansions after the side panels close", () => {
    mocks.inspectorOpen = true;
    mocks.currentTab = { type: "sessions" };
    mocks.leftSidebarExpanded = false;
    mockPanelWidths({
      bodyPanelWidth: 460,
      leftSidebarWidth: 0,
      rightPanelWidth: 360,
    });

    const renderPanels = () => (
      <MainChatPanels>
        <div data-chat-floating-anchor>
          <div data-session-surface />
        </div>
      </MainChatPanels>
    );
    const { rerender } = render(renderPanels());

    expect(mocks.windowExpandWidth).toHaveBeenCalledWith(
      40,
      null,
      false,
      false,
      true,
    );

    mocks.inspectorOpen = false;
    rerender(renderPanels());

    expect(mocks.windowRestoreWidth).toHaveBeenCalledTimes(1);
  });

  it("collapses the left sidebar when a window resize would make the note surface narrower than 500px", () => {
    mocks.currentTab = { type: "sessions" };
    mocks.leftSidebarExpanded = true;
    const panelWidths = {
      bodyPanelWidth: 720,
      leftSidebarWidth: 200,
    };
    mockPanelWidths(panelWidths);

    render(
      <MainChatPanels>
        <div data-left-sidebar-chrome />
        <div data-chat-floating-anchor>
          <div data-session-surface />
        </div>
      </MainChatPanels>,
    );

    expect(mocks.setLeftSidebarExpanded).not.toHaveBeenCalled();

    panelWidths.bodyPanelWidth = 690;
    fireEvent.resize(window);

    expect(mocks.setLeftSidebarExpanded).toHaveBeenCalledWith(false);
  });
});

function mockPanelWidths(widths: {
  bodyPanelWidth: number;
  leftSidebarWidth: number;
  rightPanelWidth?: number;
}) {
  restorePanelWidths?.();
  const spy = vi
    .spyOn(HTMLElement.prototype, "getBoundingClientRect")
    .mockImplementation(function getBoundingClientRectMock(this: HTMLElement) {
      if (this.hasAttribute("data-main-body-panel-container")) {
        return rectWithWidth(widths.bodyPanelWidth);
      }

      if (this.hasAttribute("data-left-sidebar-chrome")) {
        return rectWithWidth(widths.leftSidebarWidth);
      }

      if (this.hasAttribute("data-chat-right-panel")) {
        return rectWithWidth(widths.rightPanelWidth ?? 0);
      }

      return rectWithWidth(0);
    });
  restorePanelWidths = () => spy.mockRestore();
}

function rectWithWidth(width: number) {
  return {
    bottom: 0,
    height: 0,
    left: 0,
    right: width,
    top: 0,
    width,
    x: 0,
    y: 0,
    toJSON: () => ({}),
  };
}

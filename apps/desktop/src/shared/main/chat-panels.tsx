import { isTauri } from "@tauri-apps/api/core";
import { useCallback, useLayoutEffect, useRef } from "react";

import { commands as windowsCommands } from "@hypr/plugin-windows";
import { cn } from "@hypr/utils";

import {
  NOTE_SURFACE_MIN_WIDTH_PX,
  usesNoteSurfaceMinWidth,
} from "./layout-widths";

import { ChatPanelFrame, ChatSessionHost } from "~/chat/components/chat-panel";
import { useShell } from "~/contexts/shell";
import { type Tab, useTabs } from "~/store/zustand/tabs";

const INSPECTOR_WIDTH_PX = 360;
const LEFT_SIDEBAR_MIN_WIDTH_PX = 200;

export function MainChatPanels({ children }: { children: React.ReactNode }) {
  const { inspector, leftsidebar } = useShell();
  const currentTab = useTabs((state) => state.currentTab);
  const bodyPanelContainerRef = useRef<HTMLDivElement>(null);
  const reserveNoteSurfaceMinWidth = usesNoteSurfaceMinWidth(currentTab);
  const collapseLeftSidebar = useCallback(() => {
    leftsidebar.setExpanded(false);
  }, [leftsidebar.setExpanded]);
  const bodyMinWidth = getMainBodyMinWidth({
    currentTab,
    leftSidebarExpanded: leftsidebar.expanded,
  });

  useNoteSurfaceWindowWidthGuard({
    bodyPanelContainerRef,
    enabled: reserveNoteSurfaceMinWidth,
    leftPanelOpen: leftsidebar.expanded,
    collapseLeftPanel: collapseLeftSidebar,
    rightPanelOpen: inspector.open,
  });

  return (
    <ChatSessionHost>
      {(sessionProps) => (
        <div className="flex min-h-0 flex-1 overflow-hidden">
          <div
            ref={bodyPanelContainerRef}
            data-main-body-panel-container
            className="h-full min-h-0 min-w-0 flex-1 overflow-hidden"
            style={{ minWidth: bodyMinWidth }}
          >
            {children}
          </div>
          <div
            data-chat-right-panel
            aria-hidden={!inspector.open}
            className={cn([
              "border-border bg-card min-h-0 shrink-0 overflow-hidden",
              "transition-[width] duration-200 ease-out",
              inspector.open ? "border-l" : "border-l-0",
            ])}
            style={{ width: inspector.open ? INSPECTOR_WIDTH_PX : 0 }}
          >
            <div
              className="h-full min-h-0"
              style={{ width: INSPECTOR_WIDTH_PX }}
            >
              <ChatPanelFrame sessionProps={sessionProps} />
            </div>
          </div>
        </div>
      )}
    </ChatSessionHost>
  );
}

function getMainBodyMinWidth({
  currentTab,
  leftSidebarExpanded,
}: {
  currentTab: Tab | null;
  leftSidebarExpanded: boolean;
}) {
  if (!usesNoteSurfaceMinWidth(currentTab)) {
    return undefined;
  }

  return (
    NOTE_SURFACE_MIN_WIDTH_PX +
    (leftSidebarExpanded ? LEFT_SIDEBAR_MIN_WIDTH_PX : 0)
  );
}

function useNoteSurfaceWindowWidthGuard({
  bodyPanelContainerRef,
  collapseLeftPanel,
  enabled,
  leftPanelOpen,
  rightPanelOpen,
}: {
  bodyPanelContainerRef: React.RefObject<HTMLDivElement | null>;
  collapseLeftPanel: () => void;
  enabled: boolean;
  leftPanelOpen: boolean;
  rightPanelOpen: boolean;
}) {
  const restorableExpansionCountRef = useRef(0);
  const lastVisibleBodyWidthRef = useRef<number | null>(null);
  const previousStateRef = useRef({
    enabled: false,
    leftPanelOpen: false,
    rightPanelOpen: false,
  });

  const restoreWidthExpansions = useCallback(() => {
    const restoreCount = restorableExpansionCountRef.current;
    restorableExpansionCountRef.current = 0;

    for (let i = 0; i < restoreCount; i += 1) {
      void windowsCommands.windowRestoreWidth();
    }
  }, []);

  useLayoutEffect(() => {
    const previousState = previousStateRef.current;
    const hasOpenPanel = enabled && (leftPanelOpen || rightPanelOpen);

    if (!hasOpenPanel) {
      previousStateRef.current = { enabled, leftPanelOpen, rightPanelOpen };
      restoreWidthExpansions();
      return;
    }

    const leftPanelJustOpened =
      leftPanelOpen && (!previousState.enabled || !previousState.leftPanelOpen);
    const rightPanelJustOpened =
      rightPanelOpen &&
      (!previousState.enabled || !previousState.rightPanelOpen);

    previousStateRef.current = { enabled, leftPanelOpen, rightPanelOpen };

    if (!leftPanelJustOpened && !rightPanelJustOpened) {
      return;
    }

    const bodyPanel = bodyPanelContainerRef.current;
    if (!bodyPanel) {
      return;
    }

    const bodyWidth = getVisibleBodyWidth(bodyPanel);
    if (bodyWidth <= 0) {
      return;
    }

    let leftSidebarWidth = getLeftSidebarWidth(bodyPanel, leftPanelOpen);
    const rightPanelWidth = getRightPanelWidth(bodyPanel, rightPanelOpen);
    const shouldCollapseLeftPanelForRightPanel =
      rightPanelJustOpened &&
      leftPanelOpen &&
      leftSidebarWidth > 0 &&
      bodyWidth - leftSidebarWidth < NOTE_SURFACE_MIN_WIDTH_PX;

    if (shouldCollapseLeftPanelForRightPanel) {
      collapseLeftPanel();
      leftSidebarWidth = 0;
    }

    if (!isTauri()) {
      return;
    }

    const requiredBodyWidth = NOTE_SURFACE_MIN_WIDTH_PX + leftSidebarWidth;
    const requiredTotalWidth =
      requiredBodyWidth + (rightPanelOpen ? INSPECTOR_WIDTH_PX : 0);
    const visibleTotalWidth = bodyWidth + rightPanelWidth;
    const widthDeficit = Math.ceil(
      Math.max(
        requiredBodyWidth - bodyWidth,
        requiredTotalWidth - visibleTotalWidth,
        rightPanelOpen ? INSPECTOR_WIDTH_PX - rightPanelWidth : 0,
      ),
    );

    if (widthDeficit <= 0) {
      return;
    }

    const expandLeft = leftPanelJustOpened && !rightPanelJustOpened;
    const restoreOnClose = !expandLeft;

    if (restoreOnClose) {
      restorableExpansionCountRef.current += 1;
    }

    void windowsCommands.windowExpandWidth(
      widthDeficit,
      null,
      false,
      expandLeft,
      restoreOnClose,
    );
  }, [
    bodyPanelContainerRef,
    collapseLeftPanel,
    enabled,
    leftPanelOpen,
    restoreWidthExpansions,
    rightPanelOpen,
  ]);

  useLayoutEffect(() => {
    lastVisibleBodyWidthRef.current = null;

    if (!enabled || !leftPanelOpen) {
      return;
    }

    const bodyPanel = bodyPanelContainerRef.current;
    if (!bodyPanel) {
      return;
    }

    const handleResize = () => {
      collapseLeftPanelIfNoteSurfaceWouldShrink({
        bodyPanel,
        collapseLeftPanel,
        lastVisibleBodyWidthRef,
      });
    };

    handleResize();
    window.addEventListener("resize", handleResize);

    const resizeObserver =
      typeof ResizeObserver !== "undefined"
        ? new ResizeObserver(handleResize)
        : null;
    resizeObserver?.observe(bodyPanel);

    const shell = bodyPanel.closest<HTMLElement>(
      "[data-testid='main-app-shell']",
    );
    if (shell) {
      resizeObserver?.observe(shell);
    }

    return () => {
      window.removeEventListener("resize", handleResize);
      resizeObserver?.disconnect();
    };
  }, [
    bodyPanelContainerRef,
    collapseLeftPanel,
    enabled,
    leftPanelOpen,
    rightPanelOpen,
  ]);

  useLayoutEffect(() => restoreWidthExpansions, [restoreWidthExpansions]);
}

function collapseLeftPanelIfNoteSurfaceWouldShrink({
  bodyPanel,
  collapseLeftPanel,
  lastVisibleBodyWidthRef,
}: {
  bodyPanel: HTMLElement;
  collapseLeftPanel: () => void;
  lastVisibleBodyWidthRef: React.MutableRefObject<number | null>;
}) {
  const visibleBodyWidth = getVisibleBodyWidth(bodyPanel);
  if (visibleBodyWidth <= 0) {
    return;
  }

  const lastVisibleBodyWidth = lastVisibleBodyWidthRef.current;
  lastVisibleBodyWidthRef.current = visibleBodyWidth;

  if (
    lastVisibleBodyWidth === null ||
    visibleBodyWidth >= lastVisibleBodyWidth
  ) {
    return;
  }

  const leftSidebarWidth = getLeftSidebarWidth(bodyPanel, true);
  const noteSurfaceWidth = visibleBodyWidth - leftSidebarWidth;

  if (noteSurfaceWidth < NOTE_SURFACE_MIN_WIDTH_PX) {
    collapseLeftPanel();
  }
}

function getVisibleBodyWidth(bodyPanel: HTMLElement) {
  const bodyWidth = bodyPanel.getBoundingClientRect().width;
  const shell = bodyPanel.closest<HTMLElement>(
    "[data-testid='main-app-shell']",
  );
  if (!shell) {
    return bodyWidth;
  }

  const shellWidth = shell.getBoundingClientRect().width;
  if (shellWidth <= 0) {
    return bodyWidth;
  }

  const rightPanel = shell.querySelector<HTMLElement>(
    "[data-chat-right-panel]",
  );
  const rightPanelWidth = rightPanel?.getBoundingClientRect().width ?? 0;
  const visibleShellBodyWidth = Math.max(0, shellWidth - rightPanelWidth);

  if (bodyWidth <= 0) {
    return visibleShellBodyWidth;
  }

  return Math.min(bodyWidth, visibleShellBodyWidth);
}

function getRightPanelWidth(bodyPanel: HTMLElement, rightPanelOpen: boolean) {
  if (!rightPanelOpen) {
    return 0;
  }

  const rightPanel = bodyPanel.ownerDocument.querySelector<HTMLElement>(
    "[data-chat-right-panel]",
  );

  return rightPanel?.getBoundingClientRect().width ?? 0;
}

function getLeftSidebarWidth(bodyPanel: HTMLElement, leftPanelOpen: boolean) {
  if (!leftPanelOpen) {
    return 0;
  }

  const leftSidebarChrome = bodyPanel.querySelector<HTMLElement>(
    "[data-left-sidebar-chrome]",
  );
  const measuredWidth = leftSidebarChrome?.getBoundingClientRect().width ?? 0;

  return measuredWidth > 0 ? measuredWidth : LEFT_SIDEBAR_MIN_WIDTH_PX;
}

import { createFileRoute } from "@tanstack/react-router";
import { platform } from "@tauri-apps/plugin-os";
import { ArrowLeftIcon, ArrowRightIcon, HouseIcon } from "lucide-react";
import { Reorder } from "motion/react";
import { useCallback, useEffect, useMemo, useRef } from "react";
import { useShallow } from "zustand/shallow";

import { Button } from "@hypr/ui/components/ui/button";
import {
  type ImperativePanelHandle,
  ResizableHandle,
  ResizablePanel,
  ResizablePanelGroup,
} from "@hypr/ui/components/ui/resizable";
import { cn } from "@hypr/utils";

import { PersistentChatPanel } from "~/chat/components/persistent-chat";
import { useShell } from "~/contexts/shell";
import {
  MainTabContent,
  MainTabItem,
  useMainTabsShortcuts,
} from "~/shared/main";
import { useNewNoteAndListen } from "~/shared/main/useNewNote";
import { TrafficLights } from "~/shared/ui/traffic-lights";
import { type Tab, uniqueIdfromTab, useTabs } from "~/store/zustand/tabs";
import { useListener } from "~/stt/contexts";

export const Route = createFileRoute("/app/main2/_layout/")({
  component: Component,
});

const CHAT_MIN_WIDTH_PX = 280;

function Component() {
  const currentPlatform = platform();
  const isLinux = currentPlatform === "linux";
  const {
    tabs,
    currentTab,
    select,
    close,
    reorder,
    goBack,
    goNext,
    canGoBack,
    canGoNext,
    closeOthers,
    closeAll,
    pin,
    unpin,
    openNew,
    pendingCloseConfirmationTab,
    setPendingCloseConfirmationTab,
  } = useTabs(
    useShallow((state) => ({
      tabs: state.tabs,
      currentTab: state.currentTab,
      select: state.select,
      close: state.close,
      reorder: state.reorder,
      goBack: state.goBack,
      goNext: state.goNext,
      canGoBack: state.canGoBack,
      canGoNext: state.canGoNext,
      openNew: state.openNew,
      closeOthers: state.closeOthers,
      closeAll: state.closeAll,
      pin: state.pin,
      unpin: state.unpin,
      pendingCloseConfirmationTab: state.pendingCloseConfirmationTab,
      setPendingCloseConfirmationTab: state.setPendingCloseConfirmationTab,
    })),
  );
  const visibleTabs = useMemo(
    () => tabs.filter((tab) => tab.type !== "daily"),
    [tabs],
  );
  const homeTab = useMemo(
    () => tabs.find((tab) => tab.type === "daily") ?? null,
    [tabs],
  );
  const setTabRef = useScrollActiveTabIntoView(visibleTabs);
  const { chat } = useShell();
  const previousModeRef = useRef(chat.mode);
  const bodyPanelRef = useRef<ImperativePanelHandle>(null);
  const chatPanelContainerRef = useRef<HTMLDivElement>(null);
  const stop = useListener((state) => state.stop);
  const isRecording = useListener((state) => {
    return state.live.status === "active" || state.live.status === "finalizing";
  });
  const newNoteAndListen = useNewNoteAndListen();
  const isHomeActive = currentTab?.type === "daily";
  const isChatOpen =
    chat.mode === "FloatingOpen" || chat.mode === "RightPanelOpen";
  const isRightPanelOpen = chat.mode === "RightPanelOpen";

  useMainTabsShortcuts();

  useEffect(() => {
    const isOpeningRightPanel =
      chat.mode === "RightPanelOpen" &&
      previousModeRef.current !== "RightPanelOpen";

    if (isOpeningRightPanel && bodyPanelRef.current) {
      const currentSize = bodyPanelRef.current.getSize();
      bodyPanelRef.current.resize(currentSize);
    }

    previousModeRef.current = chat.mode;
  }, [chat.mode]);

  const handleHome = useCallback(() => {
    openNew({ type: "daily" }, { position: "start" });
  }, [openNew]);

  const handleRecord = useCallback(() => {
    if (isRecording) {
      stop();
      return;
    }

    newNoteAndListen();
  }, [isRecording, newNoteAndListen, stop]);

  const handleChat = useCallback(() => {
    chat.sendEvent(isChatOpen ? { type: "TOGGLE" } : { type: "OPEN" });
  }, [chat, isChatOpen]);

  const handleVisibleTabsReorder = useCallback(
    (reorderedTabs: Tab[]) => {
      reorder(homeTab ? [homeTab, ...reorderedTabs] : reorderedTabs);
    },
    [homeTab, reorder],
  );

  const shortcutIndexes = useMemo(() => {
    return new Map(
      visibleTabs.map((tab, index) => [
        uniqueIdfromTab(tab),
        index < 8
          ? index + 1
          : index === visibleTabs.length - 1
            ? 9
            : undefined,
      ]),
    );
  }, [visibleTabs]);

  if (!currentTab) {
    return null;
  }

  return (
    <div className="flex h-full overflow-hidden bg-stone-50 p-1">
      <div className="flex h-full min-w-0 flex-1 flex-col">
        <div
          data-tauri-drag-region
          className="flex h-9 w-full min-w-0 shrink-0 items-center gap-1 px-3"
        >
          <div
            className={cn([
              "flex shrink-0 items-center gap-1",
              isLinux ? "mr-1" : "pl-16",
            ])}
          >
            {isLinux && <TrafficLights className="mr-1" />}
            <Button
              onClick={handleHome}
              variant="ghost"
              size="icon"
              className={cn([
                "text-neutral-600",
                isHomeActive &&
                  "bg-neutral-200 text-neutral-900 hover:bg-neutral-200",
              ])}
              aria-pressed={isHomeActive}
              title="Home"
            >
              <HouseIcon size={16} />
            </Button>
            <Button
              onClick={goBack}
              disabled={!canGoBack}
              variant="ghost"
              size="icon"
              className="text-neutral-600"
            >
              <ArrowLeftIcon size={16} />
            </Button>
            <Button
              onClick={goNext}
              disabled={!canGoNext}
              variant="ghost"
              size="icon"
              className="text-neutral-600"
            >
              <ArrowRightIcon size={16} />
            </Button>
          </div>

          <div className="relative h-full min-w-0 flex-1">
            <div
              data-tauri-drag-region
              className={cn([
                "[-ms-overflow-style:none] [scrollbar-width:none] [&::-webkit-scrollbar]:hidden",
                "h-full w-full overflow-x-auto overflow-y-hidden",
              ])}
            >
              <Reorder.Group
                as="div"
                axis="x"
                values={visibleTabs}
                onReorder={handleVisibleTabsReorder}
                className="flex h-full w-max gap-1"
              >
                {visibleTabs.map((tab) => (
                  <Reorder.Item
                    key={uniqueIdfromTab(tab)}
                    value={tab}
                    as="div"
                    ref={(el) => setTabRef(tab, el)}
                    style={{ position: "relative" }}
                    className="z-10 h-full"
                    transition={{ layout: { duration: 0.15 } }}
                  >
                    <MainTabItem
                      tab={tab}
                      handleClose={close}
                      handleSelect={select}
                      handleCloseOthersCallback={closeOthers}
                      handleCloseAll={closeAll}
                      handlePin={pin}
                      handleUnpin={unpin}
                      tabIndex={shortcutIndexes.get(uniqueIdfromTab(tab))}
                      pendingCloseConfirmationTab={pendingCloseConfirmationTab}
                      setPendingCloseConfirmationTab={
                        setPendingCloseConfirmationTab
                      }
                    />
                  </Reorder.Item>
                ))}
              </Reorder.Group>
            </div>
          </div>

          <div className="ml-auto flex shrink-0 items-center gap-1">
            <button
              type="button"
              onClick={handleRecord}
              className="group flex h-5 w-5 items-center justify-center"
              title={isRecording ? "Stop recording" : "Start recording"}
            >
              <span
                className={cn([
                  "relative h-3.5 w-3.5 overflow-hidden border transition-all",
                  isRecording
                    ? [
                        "rounded-[3px]",
                        "border-red-700/60 bg-linear-to-b from-red-500 to-red-600",
                        "shadow-[inset_0_1px_0_rgba(255,255,255,0.18),0_0_0_3px_rgba(239,68,68,0.12)]",
                      ]
                    : [
                        "rounded-full",
                        "border-red-700/60 bg-linear-to-b from-red-400 to-red-500",
                        "shadow-[inset_0_1px_0_rgba(255,255,255,0.22),0_1px_2px_rgba(127,29,29,0.14)]",
                        "group-hover:from-red-400 group-hover:to-red-500 group-hover:shadow-[inset_0_1px_0_rgba(255,255,255,0.24),0_1px_2px_rgba(127,29,29,0.18)]",
                      ],
                ])}
              >
                <span className="pointer-events-none absolute top-[1px] left-1/2 h-[22%] w-[68%] -translate-x-1/2 rounded-full bg-white/18" />
              </span>
            </button>
            {chat.mode !== "FullTab" && (
              <Button
                onClick={handleChat}
                variant="ghost"
                size="icon"
                className={cn([
                  "text-neutral-600",
                  isChatOpen &&
                    "bg-neutral-200 text-neutral-900 hover:bg-neutral-200",
                ])}
                aria-label={isChatOpen ? "Close chat" : "Chat with notes"}
                aria-pressed={isChatOpen}
                title={isChatOpen ? "Close chat" : "Chat with notes"}
              >
                <img
                  src="/assets/char-logo-icon-black.svg"
                  alt="Char"
                  className={cn([
                    "size-[13px] shrink-0 object-contain opacity-65",
                    isChatOpen && "opacity-100",
                  ])}
                />
              </Button>
            )}
          </div>
        </div>

        <ResizablePanelGroup
          direction="horizontal"
          className="flex min-h-0 flex-1 overflow-hidden"
          autoSaveId="main2-chat"
        >
          <ResizablePanel
            ref={bodyPanelRef}
            className="min-h-0 flex-1 overflow-hidden"
          >
            <div className="h-full min-h-0 overflow-auto">
              <MainTabContent
                key={uniqueIdfromTab(currentTab)}
                tab={currentTab}
              />
            </div>
          </ResizablePanel>
          {isRightPanelOpen && (
            <>
              <ResizableHandle className="w-0" />
              <ResizablePanel
                defaultSize={30}
                minSize={20}
                maxSize={50}
                className="min-h-0 overflow-hidden"
                style={{ minWidth: CHAT_MIN_WIDTH_PX }}
              >
                <div
                  ref={chatPanelContainerRef}
                  className="mx-2 -mb-1 h-[calc(100%+0.25rem)] min-h-0 overflow-hidden"
                />
              </ResizablePanel>
            </>
          )}
        </ResizablePanelGroup>
      </div>
      <PersistentChatPanel panelContainerRef={chatPanelContainerRef} />
    </div>
  );
}

function useScrollActiveTabIntoView(tabs: Tab[]) {
  const tabRefsMap = useRef<Map<string, HTMLDivElement>>(new Map());
  const activeTab = tabs.find((tab) => tab.active);
  const activeTabKey = activeTab ? uniqueIdfromTab(activeTab) : null;

  useEffect(() => {
    if (!activeTabKey) {
      return;
    }

    const tabElement = tabRefsMap.current.get(activeTabKey);
    if (!tabElement) {
      return;
    }

    tabElement.scrollIntoView({
      behavior: "smooth",
      inline: "nearest",
      block: "nearest",
    });
  }, [activeTabKey]);

  return useCallback((tab: Tab, el: HTMLDivElement | null) => {
    const key = uniqueIdfromTab(tab);
    if (el) {
      tabRefsMap.current.set(key, el);
    } else {
      tabRefsMap.current.delete(key);
    }
  }, []);
}

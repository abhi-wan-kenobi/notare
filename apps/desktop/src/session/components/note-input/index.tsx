import { Trans } from "@lingui/react/macro";
import {
  forwardRef,
  type UIEventHandler,
  useCallback,
  useDeferredValue,
  useEffect,
  useImperativeHandle,
  useRef,
} from "react";
import { useHotkeys } from "react-hotkeys-hook";

import type { NoteEditorRef } from "@hypr/editor/note";
import { cn } from "@hypr/utils";

import { Enhanced } from "./enhanced";
import { Header, useEditorTabs } from "./header";
import { RawEditor } from "./raw";
import { SearchBar } from "./search/bar";
import { useSearch } from "./search/context";
import { Transcript } from "./transcript";

import { useCurrentNoteTab } from "~/session/components/shared";
import { useScrollPreservation } from "~/shared/hooks/useScrollPreservation";
import type { SessionMode } from "~/store/zustand/listener/general";
import { type Tab, useTabs } from "~/store/zustand/tabs";
import { type EditorView as TabEditorView } from "~/store/zustand/tabs/schema";
import { useListener } from "~/stt/contexts";

export interface NoteInputHandle {
  focus: () => void;
  focusAtStart: () => void;
  focusAtPixelWidth: (pixelWidth: number) => void;
  insertAtStartAndFocus: (content: string) => void;
  prepareForTabChange: () => void;
}

type NoteInputProps = {
  tab: Extract<Tab, { type: "sessions" }>;
  rawMd: string;
  sessionTitle: string;
  onNavigateToTitle?: (pixelWidth?: number) => void;
  onScroll?: UIEventHandler<HTMLDivElement>;
  editorTabs?: TabEditorView[];
  currentTab?: TabEditorView;
  handleTabChange?: (view: TabEditorView) => void;
  hideHeader?: boolean;
  embedded?: boolean;
  sessionMode?: SessionMode;
};

export function shouldShowTranscriptTabSpinner(sessionMode: SessionMode) {
  return sessionMode === "finalizing" || sessionMode === "running_batch";
}

export const NoteInput = forwardRef<NoteInputHandle, NoteInputProps>(
  function NoteInput(props, ref) {
    if (
      props.editorTabs &&
      props.currentTab &&
      props.handleTabChange &&
      props.sessionMode !== undefined
    ) {
      return (
        <NoteInputContent
          {...props}
          ref={ref}
          editorTabs={props.editorTabs}
          currentTab={props.currentTab}
          commitTabChange={props.handleTabChange}
          sessionMode={props.sessionMode}
        />
      );
    }

    return <NoteInputWithDerivedState {...props} ref={ref} />;
  },
);

const NoteInputWithDerivedState = forwardRef<NoteInputHandle, NoteInputProps>(
  function NoteInputWithDerivedState(
    { tab, editorTabs, currentTab, handleTabChange, ...props },
    ref,
  ) {
    const fallbackEditorTabs = useEditorTabs({ sessionId: tab.id });
    const fallbackCurrentTab: TabEditorView = useCurrentNoteTab(tab);
    const updateSessionTabState = useTabs(
      (state) => state.updateSessionTabState,
    );
    const tabRef = useRef(tab);
    tabRef.current = tab;
    const sessionMode = useListener((state) => state.getSessionMode(tab.id));

    const commitTabChange = useCallback(
      (tabView: TabEditorView) => {
        if (handleTabChange) {
          handleTabChange(tabView);
          return;
        }

        updateSessionTabState(tabRef.current, {
          ...tabRef.current.state,
          view: tabView,
        });
      },
      [handleTabChange, updateSessionTabState],
    );

    return (
      <NoteInputContent
        {...props}
        ref={ref}
        tab={tab}
        editorTabs={editorTabs ?? fallbackEditorTabs}
        currentTab={currentTab ?? fallbackCurrentTab}
        commitTabChange={commitTabChange}
        sessionMode={props.sessionMode ?? sessionMode}
      />
    );
  },
);

const NoteInputContent = forwardRef<
  NoteInputHandle,
  Omit<NoteInputProps, "editorTabs" | "currentTab" | "handleTabChange"> & {
    editorTabs: TabEditorView[];
    currentTab: TabEditorView;
    commitTabChange: (view: TabEditorView) => void;
    sessionMode: SessionMode;
  }
>(
  (
    {
      tab,
      rawMd,
      sessionTitle,
      onNavigateToTitle,
      onScroll,
      editorTabs,
      currentTab,
      commitTabChange,
      hideHeader = false,
      embedded = false,
      sessionMode,
    },
    ref,
  ) => {
    const internalEditorRef = useRef<NoteEditorRef>(null);
    const sessionId = tab.id;
    const deferredCurrentTab = useDeferredValue(currentTab);
    const renderedCurrentTab = editorTabs.some((editorTab) =>
      isSameEditorView(editorTab, deferredCurrentTab),
    )
      ? deferredCurrentTab
      : currentTab;

    const isMeetingInProgress =
      sessionMode === "active" ||
      sessionMode === "finalizing" ||
      sessionMode === "running_batch";
    const shouldShowTranscriptSpinner =
      shouldShowTranscriptTabSpinner(sessionMode);

    const { scrollRef, onBeforeTabChange } = useScrollPreservation(
      renderedCurrentTab.type === "enhanced"
        ? `enhanced-${renderedCurrentTab.id}`
        : renderedCurrentTab.type,
    );

    useImperativeHandle(
      ref,
      () => ({
        focus: () => internalEditorRef.current?.commands.focus(),
        focusAtStart: () => internalEditorRef.current?.commands.focusAtStart(),
        focusAtPixelWidth: (px) =>
          internalEditorRef.current?.commands.focusAtPixelWidth(px),
        insertAtStartAndFocus: (content) =>
          internalEditorRef.current?.commands.insertAtStartAndFocus(content),
        prepareForTabChange: onBeforeTabChange,
      }),
      [currentTab, onBeforeTabChange],
    );

    const handleTabChange = useCallback(
      (tabView: TabEditorView) => {
        if (
          isSameEditorView(tabView, currentTab) ||
          isSameEditorView(tabView, renderedCurrentTab)
        ) {
          return;
        }

        onBeforeTabChange();
        commitTabChange(tabView);
      },
      [commitTabChange, currentTab, onBeforeTabChange, renderedCurrentTab],
    );

    const handleAdjacentViewShortcut = useCallback(
      (direction: "previous" | "next") => {
        if (editorTabs.length <= 1) {
          return;
        }

        const currentIndex = editorTabs.findIndex((editorTab) =>
          isSameEditorView(editorTab, renderedCurrentTab),
        );
        if (currentIndex === -1) {
          return;
        }

        const nextIndex =
          direction === "previous"
            ? (currentIndex - 1 + editorTabs.length) % editorTabs.length
            : (currentIndex + 1) % editorTabs.length;
        const nextView = editorTabs[nextIndex];
        if (nextView) {
          handleTabChange(nextView);
        }
      },
      [editorTabs, handleTabChange, renderedCurrentTab],
    );

    useHotkeys(
      "mod+alt+left",
      () => handleAdjacentViewShortcut("previous"),
      {
        preventDefault: true,
        enableOnFormTags: true,
        enableOnContentEditable: true,
      },
      [handleAdjacentViewShortcut],
    );

    useHotkeys(
      "mod+alt+right",
      () => handleAdjacentViewShortcut("next"),
      {
        preventDefault: true,
        enableOnFormTags: true,
        enableOnContentEditable: true,
      },
      [handleAdjacentViewShortcut],
    );

    useEffect(() => {
      if (renderedCurrentTab.type === "raw" && isMeetingInProgress) {
        requestAnimationFrame(() => {
          internalEditorRef.current?.commands.focus();
        });
      }
    }, [renderedCurrentTab, isMeetingInProgress]);

    const search = useSearch();
    const showSearchBar = search?.isVisible ?? false;
    const isEditableTab =
      renderedCurrentTab.type === "enhanced" ||
      renderedCurrentTab.type === "raw";

    useEffect(() => {
      search?.close();
    }, [currentTab]);

    const handleContainerClick = () => {
      if (!isEditableTab) {
        return;
      }

      internalEditorRef.current?.commands.focus();
    };

    return (
      <div className={cn("-mx-2 flex flex-col", !embedded && "h-full")}>
        {!hideHeader && !embedded && (
          <div className="relative px-2">
            <Header
              sessionId={sessionId}
              editorTabs={editorTabs}
              currentTab={renderedCurrentTab}
              handleTabChange={handleTabChange}
              isTranscribing={shouldShowTranscriptSpinner}
            />
          </div>
        )}
        {embedded && renderedCurrentTab.type === "raw" && (
          <p className="text-muted-foreground px-3 pt-2 text-xs">
            <Trans>
              Start typing your meeting notes. Changes are saved automatically.
            </Trans>
          </p>
        )}

        {showSearchBar && isEditableTab && (
          <div className="px-3 pt-1">
            <SearchBar editorRef={internalEditorRef} />
          </div>
        )}

        <div className={cn("relative flex-1", !embedded && "overflow-hidden")}>
          <div
            ref={scrollRef}
            onClick={handleContainerClick}
            onScroll={onScroll}
            className={cn([
              "px-3",
              embedded ? "min-h-48" : "h-full",
              "pt-2",
              embedded
                ? "pb-6"
                : renderedCurrentTab.type === "transcript"
                  ? "overflow-hidden pb-0"
                  : "overflow-auto pb-6",
            ])}
          >
            {renderedCurrentTab.type === "enhanced" && (
              <Enhanced
                ref={internalEditorRef}
                sessionId={sessionId}
                sessionTitle={sessionTitle}
                enhancedNoteId={renderedCurrentTab.id}
                onNavigateToTitle={onNavigateToTitle}
              />
            )}
            {renderedCurrentTab.type === "raw" && (
              <RawEditor
                ref={internalEditorRef}
                sessionId={sessionId}
                rawMd={rawMd}
                sessionTitle={sessionTitle}
                onNavigateToTitle={onNavigateToTitle}
              />
            )}
            {renderedCurrentTab.type === "transcript" && (
              <Transcript sessionId={sessionId} scrollRef={scrollRef} />
            )}
          </div>
        </div>
      </div>
    );
  },
);

function isSameEditorView(left: TabEditorView, right: TabEditorView): boolean {
  if (left.type !== right.type) {
    return false;
  }

  if (left.type === "enhanced" && right.type === "enhanced") {
    return left.id === right.id;
  }

  return true;
}

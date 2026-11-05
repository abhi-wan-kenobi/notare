import { Trans } from "@lingui/react/macro";
import { useQuery } from "@tanstack/react-query";
import { convertFileSrc } from "@tauri-apps/api/core";
import React, { useEffect, useRef, useState } from "react";

import { json2md, parseJsonContent } from "@hypr/editor/markdown";
import { commands as fsSyncCommands } from "@hypr/plugin-fs-sync";

import { ActionItemsPanel } from "./components/action-items";
import { NoteInput, type NoteInputHandle } from "./components/note-input";
import { createEditorTabs } from "./components/note-input/header";
import { SearchProvider } from "./components/note-input/search/context";
import { OuterHeader } from "./components/outer-header";
import { SessionSurface } from "./components/session-surface";
import {
  computeCurrentNoteTab,
  getCanShowTranscript,
  useHasTranscript,
} from "./components/shared";
import { useAutoEnhance } from "./hooks/useAutoEnhance";
import {
  useEnhancedNotes,
  useEnsureDefaultSummaryFromState,
} from "./hooks/useEnhancedNotes";
import { MeetingDetail, type MeetingDetailTab } from "./meeting-detail";
import { useInvalidateAudioOnRecordingFinish } from "./use-invalidate-audio-on-finish";

import * as AudioPlayer from "~/audio-player";
import { useSessionEvent } from "~/session/hooks/useSessionEvent";
import {
  useEnhancedNoteRecords,
  useSession,
  useSessionParticipants,
} from "~/session/queries";
import { seekTo as seekSessionTo, useSessionStore } from "~/session/seek";
import { Dock } from "~/shared/dock";
import { type Tab, useTabs } from "~/store/zustand/tabs";
import { useListener } from "~/stt/contexts";
import { consumePendingUpload } from "~/stt/pending-upload";
import { useStartListening } from "~/stt/useStartListening";
import { useSTTConnection } from "~/stt/useSTTConnection";
import { useUploadFile } from "~/stt/useUploadFile";

export function TabContentNote({
  standaloneWindow = false,
  tab,
}: {
  standaloneWindow?: boolean;
  tab: Extract<Tab, { type: "sessions" }>;
}) {
  useInvalidateAudioOnRecordingFinish(tab.id);
  const sessionMode = useListener((state) => state.getSessionMode(tab.id));
  const audioExists = AudioPlayer.useAudioExists(tab.id);

  const audioUrlQuery = useQuery({
    enabled: sessionMode !== "active" && sessionMode !== "finalizing",
    queryKey: ["audio", tab.id, "url"],
    queryFn: () => fsSyncCommands.audioPath(tab.id),
    select: (result) => {
      if (result.status === "error") {
        return null;
      }
      return convertFileSrc(result.data);
    },
  });
  const audioUrl = audioUrlQuery.data;

  return (
    <>
      {tab.state.autoStart && !standaloneWindow ? (
        <AutoStartListening tab={tab} />
      ) : null}
      <SearchProvider>
        <AudioPlayer.Provider sessionId={tab.id} url={audioUrl ?? ""}>
          <TabContentNoteInner
            tab={tab}
            standaloneWindow={standaloneWindow}
            audioExists={audioExists}
          />
        </AudioPlayer.Provider>
      </SearchProvider>
    </>
  );
}

function AutoStartListening({
  tab,
}: {
  tab: Extract<Tab, { type: "sessions" }>;
}) {
  const canStartLiveSession = useListener((state) =>
    state.canStartLiveSession(tab.id),
  );
  const updateSessionTabState = useTabs((state) => state.updateSessionTabState);
  const { conn } = useSTTConnection();
  const startListening = useStartListening(tab.id);
  const hasAttemptedAutoStart = useRef(false);

  useEffect(() => {
    if (hasAttemptedAutoStart.current) {
      return;
    }

    if (!canStartLiveSession) {
      return;
    }

    if (!conn) {
      return;
    }

    hasAttemptedAutoStart.current = true;
    startListening();
    updateSessionTabState(tab, { ...tab.state, autoStart: null });
  }, [
    tab,
    tab.state,
    canStartLiveSession,
    conn,
    startListening,
    updateSessionTabState,
  ]);

  return null;
}

function TabContentNoteInner({
  tab,
  standaloneWindow,
  audioExists,
}: {
  tab: Extract<Tab, { type: "sessions" }>;
  standaloneWindow: boolean;
  audioExists: boolean;
}) {
  const noteInputRef = React.useRef<NoteInputHandle>(null);
  const sessionId = tab.id;
  const [detailTab, setDetailTab] = useState<MeetingDetailTab>("summary");
  useEffect(() => {
    useSessionStore.getState().setActiveSessionId(sessionId);
    return () => {
      if (useSessionStore.getState().activeSessionId === sessionId) {
        useSessionStore.getState().setActiveSessionId(null);
      }
    };
  }, [sessionId]);
  usePendingUpload(sessionId);

  const hasTranscript = useHasTranscript(sessionId);
  const sessionMode = useListener((state) => state.getSessionMode(sessionId));
  const batchError = useListener((state) => state.batch[sessionId]?.error);
  const hasLiveSegments = useListener(
    (state) =>
      state.live.sessionId === sessionId && state.liveSegments.length > 0,
  );
  const canShowTranscript = getCanShowTranscript({
    audioExists,
    batchError: Boolean(batchError),
    hasLiveSegments,
    hasTranscript,
    sessionMode,
  });
  const enhancedNoteIds = useEnhancedNotes(sessionId);
  const enhancedNotes = useEnhancedNoteRecords(sessionId);
  const session = useSession(sessionId);
  const event = useSessionEvent(sessionId);
  const participants = useSessionParticipants(sessionId);
  useEnsureDefaultSummaryFromState({
    batchError: Boolean(batchError),
    enabled: session !== null,
    enhancedNoteCount: enhancedNoteIds.length,
    hasTranscript,
    sessionId,
    sessionMode,
  });
  const updateSessionTabState = useTabs((state) => state.updateSessionTabState);
  useAutoEnhance(tab);
  const isLiveSessionActive = sessionMode === "active";
  const editorTabs = React.useMemo(
    () => createEditorTabs({ enhancedNoteIds, canShowTranscript }),
    [enhancedNoteIds, canShowTranscript],
  );
  const currentView = React.useMemo(
    () =>
      computeCurrentNoteTab(
        tab.state.view ?? null,
        isLiveSessionActive,
        enhancedNoteIds,
        canShowTranscript,
      ),
    [tab.state.view, isLiveSessionActive, enhancedNoteIds, canShowTranscript],
  );
  const transcriptView =
    editorTabs.find((view) => view.type === "transcript") ?? currentView;
  const handleTabChange = React.useCallback(
    (view: typeof currentView) => {
      noteInputRef.current?.prepareForTabChange();
      updateSessionTabState(tab, { ...tab.state, view });
    },
    [tab, updateSessionTabState],
  );
  const rawNotes = session ? bodyToMarkdown(session.raw_md) : "";
  const enhancedMarkdown = enhancedNotes[0]
    ? bodyToMarkdown(enhancedNotes[0].content)
    : "";

  useEffect(() => {
    const handleSeek = (event: Event) => {
      const detail = (event as CustomEvent<{ sessionId?: string; ms?: number }>)
        .detail;
      if (
        detail &&
        typeof detail.ms === "number" &&
        (!detail.sessionId || detail.sessionId === sessionId)
      ) {
        seekSessionTo(sessionId, detail.ms);
        setDetailTab("transcript");
        requestAnimationFrame(() => {
          const target = Array.from(
            document.querySelectorAll<HTMLElement>("[data-word-start-ms]"),
          ).find((node) => Number(node.dataset.wordStartMs) >= detail.ms!);
          target?.scrollIntoView({ block: "center", behavior: "smooth" });
        });
      }
    };
    window.addEventListener("notare:seek", handleSeek);
    return () => window.removeEventListener("notare:seek", handleSeek);
  }, [sessionId]);

  const seek = React.useCallback(
    (ms: number) => seekSessionTo(sessionId, ms),
    [sessionId],
  );
  const displayedView =
    detailTab === "transcript" ? transcriptView : currentView;

  return (
    <SessionSurface
      header={
        <OuterHeader
          sessionId={sessionId}
          currentView={displayedView}
          standaloneWindow={standaloneWindow}
          showMeetingActions
          centerTitle
          title={
            <div className="text-muted-foreground flex items-center gap-1.5 text-xs font-normal">
              <span>
                <Trans>Meetings</Trans>
              </span>
              <span className="opacity-40">/</span>
              <strong className="text-foreground max-w-[240px] truncate font-medium">
                {session?.title || event?.title || (
                  <Trans>Untitled meeting</Trans>
                )}
              </strong>
            </div>
          }
        />
      }
      floatingButton={<Dock sessionId={sessionId} />}
    >
      {session ? (
        <div className="flex h-full min-h-0 flex-col">
          <MeetingDetail
            session={session}
            event={event}
            participants={participants}
            rawNotes={rawNotes}
            enhancedMarkdown={enhancedMarkdown}
            activeTab={detailTab}
            onTabChange={setDetailTab}
            onSeek={seek}
          >
            {detailTab === "transcript" ? (
              <NoteInput
                ref={noteInputRef}
                tab={tab}
                rawMd={session.raw_md}
                sessionTitle={session.title}
                editorTabs={editorTabs}
                currentTab={transcriptView}
                handleTabChange={handleTabChange}
                sessionMode={sessionMode}
                hideHeader
              />
            ) : detailTab === "action-items" ? (
              <div className="mx-auto h-full w-full max-w-[760px] overflow-y-auto px-6 pb-24">
                <ActionItemsPanel sessionId={sessionId} />
              </div>
            ) : detailTab === "recording" ? (
              <div className="mx-auto w-full max-w-[760px] px-6 pt-4">
                {audioExists ? (
                  <div className="border-border bg-card overflow-hidden rounded-xl border">
                    <AudioPlayer.Timeline contentClassName="py-2 pr-3 pl-2" />
                  </div>
                ) : (
                  <div className="border-border/60 bg-muted/20 text-muted-foreground rounded-xl border border-dashed py-12 text-center text-sm">
                    <Trans>
                      No recording audio available for this meeting.
                    </Trans>
                  </div>
                )}
              </div>
            ) : null}
          </MeetingDetail>
        </div>
      ) : (
        <SessionContentLoading />
      )}
    </SessionSurface>
  );
}
function bodyToMarkdown(content: string) {
  if (!content.trim()) return "";
  if (!content.trim().startsWith("{")) return content;
  try {
    return json2md(parseJsonContent(content));
  } catch {
    return content;
  }
}

function SessionContentLoading() {
  return (
    <div className="flex h-full flex-col gap-3 px-4 py-5">
      <div className="bg-muted h-5 w-3/5 animate-pulse rounded-md" />
      <div className="bg-muted/80 h-4 w-4/5 animate-pulse rounded-md" />
      <div className="bg-muted/70 h-4 w-2/3 animate-pulse rounded-md" />
    </div>
  );
}

function usePendingUpload(sessionId: string) {
  const { processFile } = useUploadFile(sessionId);
  const processFileRef = useRef(processFile);
  processFileRef.current = processFile;

  useEffect(() => {
    const pending = consumePendingUpload(sessionId);
    if (pending) {
      processFileRef.current(pending.filePath, pending.kind);
    }
  }, [sessionId]);
}

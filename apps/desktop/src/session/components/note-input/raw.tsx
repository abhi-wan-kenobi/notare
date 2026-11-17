import { useLingui } from "@lingui/react/macro";
import type { EditorView } from "prosemirror-view";
import { forwardRef, useCallback, useMemo, useRef } from "react";

import { parseJsonContent } from "@hypr/editor/markdown";
import {
  NoteEditor,
  type JSONContent,
  type NoteEditorRef,
} from "@hypr/editor/note";
import { commands as analyticsCommands } from "@hypr/plugin-analytics";
import { sonnerToast } from "@hypr/ui/components/ui/toast";
import { cn } from "@hypr/utils";

import { AudioDropTarget } from "./audio-drop-target";
import { useNoteFileHandlerConfig } from "./file-handler";

import { AppLinkView } from "~/editor-bridge/app-link-view";
import { useMentionConfig } from "~/editor-bridge/mention-config";
import { openEditorLink } from "~/editor-bridge/open-editor-link";
import { sessionMentionDropConfig } from "~/editor-bridge/session-mention-drop";
import { SessionNodeView } from "~/editor-bridge/session-view";
import { hasStoredNoteContent } from "~/session/components/shared";
import { useUpdateSession } from "~/session/queries";
import {
  ensureFirstLineTitle,
  extractFirstLineTitle,
} from "~/session/title-content";

const extraNodeViews = { appLink: AppLinkView, session: SessionNodeView };
// One id so repeated failed saves update a single toast instead of stacking.
const SAVE_ERROR_TOAST_ID = "raw-note-save-error";

export const RawEditor = forwardRef<
  NoteEditorRef,
  {
    sessionId: string;
    rawMd: string;
    sessionTitle: string;
    className?: string;
    onNavigateToTitle?: (pixelWidth?: number) => void;
    syncTasks?: boolean;
    showFormatToolbar?: boolean;
    onViewReady?: (view: EditorView) => void;
    onViewDisposed?: (view: EditorView) => void;
  }
>(
  (
    {
      sessionId,
      rawMd,
      sessionTitle,
      className,
      onNavigateToTitle,
      syncTasks = true,
      showFormatToolbar = true,
      onViewReady,
      onViewDisposed,
    },
    ref,
  ) => {
    const { t } = useLingui();
    const updateSession = useUpdateSession(sessionId);
    const { audioDropTargetProps, fileHandlerConfig, isAudioDragActive } =
      useNoteFileHandlerConfig(sessionId);
    const initialContent = useMemo<JSONContent>(() => {
      const content = ensureFirstLineTitle(
        parseJsonContent(rawMd),
        sessionTitle,
      );
      const blocks = content.content ?? [];
      // A titled, empty meeting still needs a body to click into and type.
      return blocks.length === 1 && blocks[0].type === "heading"
        ? { ...content, content: [...blocks, { type: "paragraph" }] }
        : content;
    }, [rawMd, sessionTitle]);
    const lastSavedContentRef = useRef<JSONContent | null>(null);

    const persistChange = useCallback(
      async (input: JSONContent) => {
        const title = extractFirstLineTitle(input);
        await updateSession({
          raw_md: JSON.stringify(input),
          ...(title !== null || hasStoredNoteContent(rawMd)
            ? { title: title ?? "" }
            : {}),
        });
        // Only a fulfilled write counts as saved. A rejected one leaves the
        // content dirty, so disposing the view writes it again.
        lastSavedContentRef.current = input;
      },
      [rawMd, updateSession],
    );

    const hasTrackedWriteRef = useRef(false);
    const trackedSessionIdRef = useRef(sessionId);
    if (trackedSessionIdRef.current !== sessionId) {
      trackedSessionIdRef.current = sessionId;
      hasTrackedWriteRef.current = false;
    }

    const hasNonEmptyText = useCallback(
      (node?: JSONContent): boolean =>
        !!node?.text?.trim() ||
        !!node?.content?.some((child: JSONContent) => hasNonEmptyText(child)),
      [],
    );

    const handleChange = useCallback(
      (input: JSONContent) => {
        void persistChange(input).catch((error) => {
          console.error("[raw-editor] failed to persist note", error);
          sonnerToast.error(
            t`Couldn't save your notes. Your latest changes are not saved yet.`,
            { id: SAVE_ERROR_TOAST_ID },
          );
        });

        if (!hasTrackedWriteRef.current) {
          const hasContent = hasNonEmptyText(input);
          if (hasContent) {
            hasTrackedWriteRef.current = true;
            void trackNoteEdited();
          }
        }
      },
      [persistChange, hasNonEmptyText, t],
    );

    const handleViewReady = useCallback(
      (view: EditorView) => {
        lastSavedContentRef.current = view.state.doc.toJSON();
        onViewReady?.(view);
      },
      [onViewReady],
    );
    const handleViewDisposed = useCallback(
      (view: EditorView) => {
        const content = view.state.doc.toJSON() as JSONContent;
        // The shared editor debounces changes. Save any final keystrokes before
        // switching views or closing a meeting cancels its pending callback.
        if (
          JSON.stringify(content) !==
          JSON.stringify(lastSavedContentRef.current)
        ) {
          handleChange(content);
        }
        onViewDisposed?.(view);
      },
      [handleChange, onViewDisposed],
    );

    const mentionConfig = useMentionConfig();

    return (
      <AudioDropTarget
        targetProps={audioDropTargetProps}
        isActive={isAudioDragActive}
      >
        <NoteEditor
          ref={ref}
          className={cn(["session-note-editor", className])}
          key={`session-${sessionId}-raw`}
          initialContent={initialContent}
          handleChange={handleChange}
          mentionConfig={mentionConfig}
          sessionMentionDropConfig={sessionMentionDropConfig}
          onNavigateToTitle={onNavigateToTitle}
          onLinkOpen={openEditorLink}
          fileHandlerConfig={fileHandlerConfig}
          taskSource={
            syncTasks ? { type: "session_raw_note", id: sessionId } : undefined
          }
          extraNodeViews={extraNodeViews}
          showFormatToolbar={showFormatToolbar}
          onViewReady={handleViewReady}
          onViewDisposed={handleViewDisposed}
        />
      </AudioDropTarget>
    );
  },
);

async function trackNoteEdited() {
  try {
    await analyticsCommands.event({
      event: "note_edited",
      has_content: true,
    });
  } catch (error) {
    console.error("[raw-editor] failed to record note analytics", error);
  }
}

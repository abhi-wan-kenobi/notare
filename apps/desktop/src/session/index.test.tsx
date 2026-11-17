import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import type { DatabaseSync } from "node:sqlite";
import type { EditorView } from "prosemirror-view";
import { useState } from "react";
import type * as React from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type * as NoteEditorModule from "@hypr/editor/note";
import type { NoteEditorProps, NoteEditorRef } from "@hypr/editor/note";

import { createMigratedDb, execAll, queryAll } from "~/calendar/test-sqlite";
import { flushDatabaseWrites } from "~/db/write-queue";
import type { SessionMode } from "~/store/zustand/listener/general";
import type { Tab } from "~/store/zustand/tabs";
import type { EditorView as NoteView } from "~/store/zustand/tabs/schema";

const state = vi.hoisted(() => ({
  db: null as unknown as DatabaseSync,
  editorView: null as EditorView | null,
  mode: "inactive" as SessionMode,
  enhancedNoteIds: [] as string[],
  updateSessionTabState: vi.fn(),
}));

vi.mock("~/db", () => ({
  executeTransaction: async (
    statements: Array<{ sql: string; params?: unknown[] }>,
  ) => execAll(state.db, statements),
  liveQueryClient: {
    execute: async (sql: string, params?: unknown[]) =>
      queryAll(state.db, sql, params),
  },
  useLiveQuery: (options: {
    sql: string;
    params?: unknown[];
    enabled?: boolean;
    mapRows: (rows: unknown[]) => unknown;
  }) => ({
    data:
      options.enabled === false
        ? undefined
        : options.mapRows(queryAll(state.db, options.sql, options.params)),
  }),
}));

vi.mock("@tanstack/react-query", () => ({ useQuery: () => ({ data: null }) }));
vi.mock("~/audio-player", () => ({
  Provider: ({ children }: { children: React.ReactNode }) => children,
  useAudioExists: () => false,
  Timeline: () => <div>Audio timeline</div>,
}));
vi.mock("./use-invalidate-audio-on-finish", () => ({
  useInvalidateAudioOnRecordingFinish: vi.fn(),
}));
vi.mock("./hooks/useAutoEnhance", () => ({ useAutoEnhance: vi.fn() }));
vi.mock("./hooks/useEnhancedNotes", () => ({
  useEnhancedNotes: () => state.enhancedNoteIds,
  useEnsureDefaultSummaryFromState: vi.fn(),
}));
vi.mock("./hooks/useSessionEvent", () => ({ useSessionEvent: () => null }));
vi.mock("./series-meta", () => ({ useSessionSeriesMeta: () => null }));
vi.mock("./components/action-items/queries", () => ({
  useSessionActionItems: () => ({ items: [] }),
}));
vi.mock("./components/action-items", () => ({
  ActionItemsPanel: () => <div>Meeting tasks</div>,
}));
vi.mock("./components/outer-header", () => ({ OuterHeader: () => null }));
vi.mock("./components/session-surface", () => ({
  SessionSurface: ({ children }: { children: React.ReactNode }) => (
    <div>{children}</div>
  ),
}));
vi.mock("~/shared/dock", () => ({ Dock: () => null }));
vi.mock("~/stt/contexts", () => ({
  useListener: (selector: (value: unknown) => unknown) =>
    selector({
      getSessionMode: () => state.mode,
      batch: {},
      live: { sessionId: null },
      liveSegments: [],
    }),
}));
vi.mock("~/stt/pending-upload", () => ({ consumePendingUpload: () => null }));
vi.mock("~/stt/useStartListening", () => ({
  useStartListening: () => vi.fn(),
}));
vi.mock("~/stt/useSTTConnection", () => ({
  useSTTConnection: () => ({ conn: null }),
}));
vi.mock("~/stt/useUploadFile", () => ({
  AUDIO_EXTENSIONS: ["wav", "mp3"],
  useUploadFile: () => ({ processAudioFile: vi.fn() }),
}));
vi.mock("~/store/zustand/tabs", () => ({
  useTabs: (selector: (value: unknown) => unknown) =>
    selector({
      updateSessionTabState: state.updateSessionTabState,
      openNew: vi.fn(),
    }),
}));
vi.mock("./components/note-input/header", () => ({
  createEditorTabs: ({
    enhancedNoteIds,
    canShowTranscript,
  }: {
    enhancedNoteIds: string[];
    canShowTranscript: boolean;
  }) => [
    ...enhancedNoteIds.map((id) => ({ type: "enhanced", id })),
    { type: "raw" },
    ...(canShowTranscript ? [{ type: "transcript" }] : []),
  ],
  Header: ({
    editorTabs,
    handleTabChange,
  }: {
    editorTabs: NoteView[];
    handleTabChange: (view: NoteView) => void;
  }) => (
    <div>
      {editorTabs.map((view) => (
        <button
          key={view.type === "enhanced" ? view.id : view.type}
          onClick={() => handleTabChange(view)}
        >
          {view.type === "enhanced"
            ? "AI summary"
            : view.type === "raw"
              ? "Memos"
              : "Transcript view"}
        </button>
      ))}
    </div>
  ),
}));
vi.mock("./components/note-input/enhanced", () => ({
  Enhanced: () => <div>Generated meeting summary</div>,
}));
vi.mock("./components/note-input/transcript", () => ({
  Transcript: () => <div>Meeting transcript</div>,
}));
vi.mock("./components/note-input/search/bar", () => ({
  SearchBar: () => null,
}));
vi.mock("./components/note-input/search/context", () => ({
  SearchProvider: ({ children }: { children: React.ReactNode }) => children,
  useSearch: () => null,
}));
vi.mock("./components/note-input/file-handler", () => ({
  useNoteFileHandlerConfig: () => ({
    audioDropTargetProps: {},
    isAudioDragActive: false,
  }),
}));
vi.mock("~/editor-bridge/app-link-view", () => ({ AppLinkView: () => null }));
vi.mock("~/editor-bridge/session-view", () => ({
  SessionNodeView: () => null,
}));
vi.mock("~/editor-bridge/mention-config", () => ({
  useMentionConfig: () => undefined,
}));
vi.mock("~/editor-bridge/open-editor-link", () => ({
  openEditorLink: vi.fn(),
}));
vi.mock("@hypr/editor/note", async (importOriginal) => {
  const actual = await importOriginal<typeof NoteEditorModule>();
  // This hoisted mock factory runs before static runtime imports initialize.
  const { forwardRef } = await vi.importActual<typeof React>("react");
  return {
    ...actual,
    NoteEditor: forwardRef<NoteEditorRef, NoteEditorProps>((props, ref) => (
      <actual.NoteEditor
        {...props}
        ref={ref}
        onViewReady={(view) => {
          state.editorView = view;
          props.onViewReady?.(view);
        }}
        onViewDisposed={(view) => {
          state.editorView = null;
          props.onViewDisposed?.(view);
        }}
      />
    )),
  };
});

import { TabContentNote } from ".";

let tab: Extract<Tab, { type: "sessions" }>;

function MeetingHarness() {
  const [sessionTab, setSessionTab] = useState(tab);
  state.updateSessionTabState.mockImplementation(
    (_tab: typeof tab, nextState: typeof tab.state) =>
      setSessionTab((current) => ({ ...current, state: nextState })),
  );
  return <TabContentNote tab={sessionTab} />;
}

function openMeeting() {
  return render(<MeetingHarness />);
}

function notesDocument() {
  return queryAll<{ body: string; body_format: string }>(
    state.db,
    "SELECT body, body_format FROM session_documents WHERE id = ? AND kind = 'note'",
    [tab.id],
  )[0];
}

async function typeNotes(text: string) {
  await waitFor(() => expect(state.editorView).not.toBeNull());
  // Dispatch the same ProseMirror transaction as text input, through the live
  // editor rather than calling its save callback or replacing it with a fake.
  act(() => {
    const view = state.editorView!;
    const lastBlock = view.state.doc.lastChild!;
    const position =
      view.state.doc.content.size - (lastBlock.isTextblock ? 1 : 0);
    view.dispatch(view.state.tr.insertText(text, position));
  });
}

beforeEach(() => {
  state.db = createMigratedDb();
  state.mode = "inactive";
  state.editorView = null;
  state.enhancedNoteIds = [];
  state.updateSessionTabState.mockReset();
  tab = {
    id: "manual-notes-session",
    type: "sessions",
    active: true,
    pinned: false,
    slotId: "slot-1",
    state: { autoStart: null, view: null },
  };
  execAll(state.db, [
    {
      sql: "INSERT INTO sessions (id, owner_user_id, title, created_at, updated_at) VALUES (?, 'user-1', 'Weekly sync', '2026-11-17T00:00:00Z', '2026-11-17T00:00:00Z')",
      params: [tab.id],
    },
    {
      sql: "INSERT INTO session_documents (id, session_id, kind, body_format, body, created_by, updated_by, created_at, updated_at) VALUES (?, ?, 'note', 'prosemirror_json', '', 'user-1', 'user-1', '2026-11-17T00:00:00Z', '2026-11-17T00:00:00Z')",
      params: [tab.id, tab.id],
    },
  ]);
});

afterEach(async () => {
  cleanup();
  await flushDatabaseWrites();
  state.db.close();
});

describe("manual meeting notes", () => {
  it.each<SessionMode>(["inactive", "active", "finalizing"])(
    "saves edits to the canonical note document without AI configuration (%s)",
    async (mode) => {
      state.mode = mode;
      openMeeting();
      expect(screen.getByRole("heading", { name: "Notes" })).not.toBeNull();
      expect(
        screen.getByText(
          "Start typing your meeting notes. Changes are saved automatically.",
        ),
      ).not.toBeNull();
      await typeNotes("Decide the launch date");
      await waitFor(() =>
        expect(notesDocument().body).toContain("Decide the launch date"),
      );
      expect(notesDocument().body_format).toBe("prosemirror_json");
      expect(JSON.parse(notesDocument().body).content).toEqual(
        expect.arrayContaining([
          expect.objectContaining({
            type: "paragraph",
            content: [{ type: "text", text: "Decide the launch date" }],
          }),
        ]),
      );
    },
  );

  it("persists the last keystrokes when switching sections immediately and reopening", async () => {
    const meeting = openMeeting();
    await typeNotes("Take this offline");
    const sections = screen.getByRole("navigation", {
      name: "Meeting sections",
    });
    fireEvent.click(
      within(sections).getByRole("button", { name: "Action items" }),
    );
    expect(screen.getByText("Meeting tasks")).not.toBeNull();
    await waitFor(() =>
      expect(notesDocument().body).toContain("Take this offline"),
    );
    fireEvent.click(within(sections).getByRole("button", { name: "Summary" }));
    expect(screen.getByRole("textbox").textContent).toContain(
      "Take this offline",
    );
    meeting.unmount();
    openMeeting();
    expect(screen.getByRole("textbox").textContent).toContain(
      "Take this offline",
    );
  });

  it("flushes unsaved typing when the meeting closes before the debounce", async () => {
    const meeting = openMeeting();
    await typeNotes("Remember the owner");
    meeting.unmount();
    await waitFor(() =>
      expect(notesDocument().body).toContain("Remember the owner"),
    );
    openMeeting();
    expect(screen.getByRole("textbox").textContent).toContain(
      "Remember the owner",
    );
  });

  it("restores rich notes without flattening marks or lists and keeps all meeting sections", async () => {
    const document = {
      type: "doc",
      content: [
        {
          type: "heading",
          attrs: { level: 1 },
          content: [{ type: "text", text: "Weekly sync" }],
        },
        {
          type: "paragraph",
          content: [
            { type: "text", text: "Important", marks: [{ type: "bold" }] },
          ],
        },
        {
          type: "bulletList",
          content: [
            {
              type: "listItem",
              content: [
                {
                  type: "paragraph",
                  content: [{ type: "text", text: "Keep the context" }],
                },
              ],
            },
          ],
        },
        { type: "paragraph" },
      ],
    };
    state.db
      .prepare("UPDATE session_documents SET body = ? WHERE id = ?")
      .run(JSON.stringify(document), tab.id);
    const meeting = openMeeting();
    expect(
      screen.getByRole("textbox").querySelector("strong")?.textContent,
    ).toBe("Important");
    expect(screen.getByRole("textbox").querySelector("li")?.textContent).toBe(
      "Keep the context",
    );
    const sections = screen.getByRole("navigation", {
      name: "Meeting sections",
    });
    expect(within(sections).getAllByRole("button")).toHaveLength(4);
    fireEvent.click(
      within(sections).getByRole("button", { name: "Transcript" }),
    );
    expect(screen.getByText("Meeting transcript")).not.toBeNull();
    fireEvent.click(
      within(sections).getByRole("button", { name: "Recording" }),
    );
    expect(
      screen.getByText("No recording audio available for this meeting."),
    ).not.toBeNull();
    fireEvent.click(within(sections).getByRole("button", { name: "Summary" }));
    await typeNotes("Added context");
    await waitFor(() =>
      expect(notesDocument().body).toContain("Added context"),
    );
    meeting.unmount();
    openMeeting();
    expect(
      screen.getByRole("textbox").querySelector("strong")?.textContent,
    ).toBe("Important");
    expect(screen.getByRole("textbox").querySelector("li")?.textContent).toBe(
      "Keep the context",
    );
  });

  it("refreshes an unfocused editor from updated canonical note data", async () => {
    const meeting = openMeeting();
    const refreshedDocument = {
      type: "doc",
      content: [
        {
          type: "heading",
          attrs: { level: 1 },
          content: [{ type: "text", text: "Weekly sync" }],
        },
        {
          type: "paragraph",
          content: [{ type: "text", text: "Notes synced from another device" }],
        },
      ],
    };
    state.db
      .prepare("UPDATE session_documents SET body = ? WHERE id = ?")
      .run(JSON.stringify(refreshedDocument), tab.id);
    meeting.rerender(<MeetingHarness />);
    await waitFor(() =>
      expect(screen.getByRole("textbox").textContent).toContain(
        "Notes synced from another device",
      ),
    );
  });

  it("opens editable notes by default while keeping generated summaries accessible", async () => {
    state.enhancedNoteIds = ["summary-1"];
    openMeeting();
    expect(screen.getByRole("textbox")).not.toBeNull();
    await typeNotes("Keep the original notes");
    fireEvent.click(screen.getByRole("button", { name: "AI summary" }));
    expect(await screen.findByText("Generated meeting summary")).not.toBeNull();
    await waitFor(() =>
      expect(notesDocument().body).toContain("Keep the original notes"),
    );
    fireEvent.click(screen.getByRole("button", { name: "Memos" }));
    expect(await screen.findByRole("textbox")).not.toBeNull();
    expect(screen.getByRole("textbox").textContent).toContain(
      "Keep the original notes",
    );
  });
});

import { beforeEach, describe, expect, test, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  sessionDir: vi.fn(),
  writeJsonBatch: vi.fn(),
  writeDocumentBatch: vi.fn(),
  subscribe: vi.fn(),
  loadSessionContentSnapshot: vi.fn(),
  buildRenderTranscriptRequestFromRows: vi.fn(),
  renderTranscriptSegments: vi.fn(),
}));

vi.mock("@hypr/plugin-fs-sync", () => ({
  commands: {
    sessionDir: mocks.sessionDir,
    writeJsonBatch: mocks.writeJsonBatch,
    writeDocumentBatch: mocks.writeDocumentBatch,
  },
}));

vi.mock("~/db", () => ({
  liveQueryClient: { subscribe: mocks.subscribe },
}));

vi.mock("~/session/content-queries", () => ({
  loadSessionContentSnapshot: mocks.loadSessionContentSnapshot,
}));

vi.mock("~/stt/render-transcript", () => ({
  buildRenderTranscriptRequestFromRows:
    mocks.buildRenderTranscriptRequestFromRows,
  renderTranscriptSegments: mocks.renderTranscriptSegments,
}));

import {
  buildFallbackTranscriptSegments,
  buildSessionFiles,
  collectChangedSessions,
  formatTimestamp,
  formatTranscriptMarkdown,
  materializeSession,
} from "./fs-materializer";

import type { SessionContentSnapshot } from "~/session/content-queries";

const SESSION_ID = "6a1c9e0e-0000-4000-8000-000000000001";
const NOTE_ID = "6a1c9e0e-0000-4000-8000-000000000002";

function makeSnapshot(
  overrides: Partial<SessionContentSnapshot> = {},
): SessionContentSnapshot {
  return {
    sessionId: SESSION_ID,
    ownerUserId: "user-1",
    title: "Weekly Sync",
    createdAt: "2026-07-14T10:00:00.000Z",
    event: null,
    eventId: null,
    rawNoteId: SESSION_ID,
    rawContent: "",
    rawContentFormat: "markdown",
    rawMarkdown: "my memo",
    enhancedNotes: [
      {
        id: NOTE_ID,
        title: "Summary",
        markdown: "## Summary\n\ndone",
        content: "",
        contentFormat: "markdown",
        templateId: "template-1",
        position: 0,
      },
    ],
    transcripts: [
      {
        id: "transcript-1",
        started_at: 0,
        ended_at: 1200,
        memo: "",
        wordsJson: "[]",
        words: [
          {
            id: "word-1",
            text: "hello",
            start_ms: 0,
            end_ms: 400,
            channel: 0,
            speaker: "Alice",
          },
          {
            id: "word-2",
            text: "there",
            start_ms: 400,
            end_ms: 800,
            channel: 0,
            speaker: "Alice",
          },
          {
            id: "word-3",
            text: "hi",
            start_ms: 900,
            end_ms: 1200,
            channel: 0,
            speaker: "Bob",
          },
        ],
        speaker_hints: [],
      },
    ],
    participants: [{ humanId: "human-1", name: "Alice", jobTitle: "PM" }],
    actionItems: [],
    ...overrides,
  };
}

describe("buildSessionFiles", () => {
  test("materializes meta, memo, notes, and transcript files", () => {
    const files = buildSessionFiles(makeSnapshot(), "# Transcript\n\nhello\n");

    const meta = files.json.find(([, name]) => name === "_meta.json");
    expect(meta?.[0]).toEqual({
      id: SESSION_ID,
      userId: "user-1",
      createdAt: "2026-07-14T10:00:00.000Z",
      title: "Weekly Sync",
      event: null,
      eventId: null,
    });

    const transcript = files.json.find(
      ([, name]) => name === "transcript.json",
    );
    expect(transcript?.[0]).toEqual({
      transcripts: [
        expect.objectContaining({
          id: "transcript-1",
          user_id: "user-1",
          session_id: SESSION_ID,
          started_at: 0,
          ended_at: 1200,
          memo_md: "",
          words: expect.arrayContaining([
            expect.objectContaining({ text: "hello" }),
          ]),
          speaker_hints: [],
        }),
      ],
    });

    const memo = files.documents.find(([, name]) => name === "_memo.md");
    expect(memo?.[0]).toEqual({
      frontmatter: {
        id: SESSION_ID,
        session_id: SESSION_ID,
        title: "Weekly Sync",
      },
      content: "my memo",
    });

    const note = files.documents.find(([, name]) => name === `${NOTE_ID}.md`);
    expect(note?.[0]).toEqual({
      frontmatter: {
        id: NOTE_ID,
        session_id: SESSION_ID,
        template_id: "template-1",
        position: 0,
        title: "Summary",
      },
      content: "## Summary\n\ndone",
    });

    const transcriptMd = files.documents.find(
      ([, name]) => name === "transcript.md",
    );
    expect(transcriptMd?.[0]).toEqual({
      frontmatter: {},
      content: "# Transcript\n\nhello\n",
    });
  });

  test("skips memo, transcript, and notes when empty", () => {
    const files = buildSessionFiles(
      makeSnapshot({
        rawNoteId: null,
        rawMarkdown: "",
        enhancedNotes: [],
        transcripts: [],
      }),
      null,
    );

    expect(files.json.map(([, name]) => name)).toEqual(["_meta.json"]);
    expect(files.documents).toEqual([]);
  });

  test("renders action items into _memo.md as an Obsidian task section", () => {
    const files = buildSessionFiles(
      makeSnapshot({
        rawMarkdown: "meeting body",
        actionItems: [
          {
            text: "Send budget",
            status: "todo",
            dueAt: "2026-07-24",
            ownerSpeakerId: "spk_1",
          },
          { text: "Book venue", status: "done", dueAt: "", ownerSpeakerId: "" },
        ],
      }),
      null,
    );

    const memo = files.documents.find(([, name]) => name === "_memo.md");
    const content = memo?.[0].content ?? "";
    expect(content).toContain("meeting body");
    expect(content).toContain("<!-- notare:action-items -->");
    expect(content).toContain("## Action Items");
    expect(content).toContain("- [ ] Send budget 📅 2026-07-24 @spk_1");
    expect(content).toContain("- [x] Book venue");
  });

  test("writes _memo.md for action items even when the note body is empty", () => {
    const files = buildSessionFiles(
      makeSnapshot({
        rawNoteId: null,
        rawMarkdown: "",
        enhancedNotes: [],
        transcripts: [],
        actionItems: [
          { text: "Only task", status: "todo", dueAt: "", ownerSpeakerId: "" },
        ],
      }),
      null,
    );

    const memo = files.documents.find(([, name]) => name === "_memo.md");
    expect(memo?.[0].content).toContain("- [ ] Only task");
  });
});

describe("transcript markdown", () => {
  test("formats segments with speaker and timestamp", () => {
    const markdown = formatTranscriptMarkdown([
      { speaker: "Alice", startMs: 0, text: "hello there" },
      { speaker: null, startMs: 65_000, text: "hi" },
      { speaker: "Bob", startMs: null, text: "   " },
    ]);

    expect(markdown).toBe(
      "# Transcript\n\n**Alice** [00:00]\nhello there\n\n**Speaker** [01:05]\nhi\n",
    );
  });

  test("returns null when there is nothing to say", () => {
    expect(formatTranscriptMarkdown([])).toBeNull();
    expect(
      formatTranscriptMarkdown([{ speaker: "A", startMs: 0, text: " " }]),
    ).toBeNull();
  });

  test("fallback segments group consecutive words by speaker", () => {
    const segments = buildFallbackTranscriptSegments(makeSnapshot());

    expect(segments).toEqual([
      { speaker: "Alice", startMs: 0, text: "hello there" },
      { speaker: "Bob", startMs: 900, text: "hi" },
    ]);
  });

  test("formats hour-long timestamps", () => {
    expect(formatTimestamp(3_723_000)).toBe("1:02:03");
    expect(formatTimestamp(59_000)).toBe("00:59");
  });
});

describe("collectChangedSessions", () => {
  test("reports new, changed, and removed sessions", () => {
    const { changed, removedIds } = collectChangedSessions(
      { a: "1", b: "1", c: "1" },
      [
        { id: "a", dirty_key: "1" },
        { id: "b", dirty_key: "2" },
        { id: "d", dirty_key: "1" },
      ],
    );

    expect(changed).toEqual([
      { id: "b", dirty_key: "2" },
      { id: "d", dirty_key: "1" },
    ]);
    expect(removedIds).toEqual(["c"]);
  });
});

describe("materializeSession", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.sessionDir.mockResolvedValue({
      status: "ok",
      data: `/vault/sessions/${SESSION_ID}`,
    });
    mocks.writeJsonBatch.mockResolvedValue({ status: "ok", data: null });
    mocks.writeDocumentBatch.mockResolvedValue({ status: "ok", data: null });
    mocks.buildRenderTranscriptRequestFromRows.mockReturnValue({
      transcripts: [],
      participant_human_ids: [],
      self_human_id: null,
      humans: [],
    });
    mocks.renderTranscriptSegments.mockResolvedValue([
      {
        id: "s1",
        key: {},
        speaker_label: "Alice",
        start_ms: 0,
        end_ms: 800,
        text: "hello there",
        words: [],
      },
    ]);
  });

  test("returns false when the session does not exist", async () => {
    mocks.loadSessionContentSnapshot.mockResolvedValue(null);

    await expect(materializeSession(SESSION_ID)).resolves.toBe(false);
    expect(mocks.writeJsonBatch).not.toHaveBeenCalled();
    expect(mocks.writeDocumentBatch).not.toHaveBeenCalled();
  });

  test("writes all files into the session directory", async () => {
    mocks.loadSessionContentSnapshot.mockResolvedValue(makeSnapshot());

    await expect(materializeSession(SESSION_ID)).resolves.toBe(true);

    const jsonPaths = mocks.writeJsonBatch.mock.calls[0][0].map(
      ([, path]: [unknown, string]) => path,
    );
    expect(jsonPaths).toEqual([
      `/vault/sessions/${SESSION_ID}/_meta.json`,
      `/vault/sessions/${SESSION_ID}/transcript.json`,
    ]);

    const documentPaths = mocks.writeDocumentBatch.mock.calls[0][0].map(
      ([, path]: [unknown, string]) => path,
    );
    expect(documentPaths).toEqual([
      `/vault/sessions/${SESSION_ID}/_memo.md`,
      `/vault/sessions/${SESSION_ID}/${NOTE_ID}.md`,
      `/vault/sessions/${SESSION_ID}/transcript.md`,
    ]);

    const transcriptDoc = mocks.writeDocumentBatch.mock.calls[0][0].find(
      ([, path]: [unknown, string]) => path.endsWith("transcript.md"),
    );
    expect(transcriptDoc[0].content).toContain("**Alice** [00:00]");
  });

  test("falls back to word grouping when the renderer fails", async () => {
    mocks.loadSessionContentSnapshot.mockResolvedValue(makeSnapshot());
    mocks.renderTranscriptSegments.mockRejectedValue(new Error("boom"));

    await expect(materializeSession(SESSION_ID)).resolves.toBe(true);

    const transcriptDoc = mocks.writeDocumentBatch.mock.calls[0][0].find(
      ([, path]: [unknown, string]) => path.endsWith("transcript.md"),
    );
    expect(transcriptDoc[0].content).toContain(
      "**Alice** [00:00]\nhello there",
    );
    expect(transcriptDoc[0].content).toContain("**Bob** [00:00]\nhi");
  });

  test("surfaces write failures", async () => {
    mocks.loadSessionContentSnapshot.mockResolvedValue(makeSnapshot());
    mocks.writeJsonBatch.mockResolvedValue({
      status: "error",
      error: "disk full",
    });

    await expect(materializeSession(SESSION_ID)).rejects.toThrow("disk full");
  });
});

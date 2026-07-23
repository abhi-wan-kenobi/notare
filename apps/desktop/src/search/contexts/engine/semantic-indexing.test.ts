import { beforeEach, describe, expect, it, vi } from "vitest";

type Sub = { onData: (rows: unknown[]) => void; onError?: (e: string) => void };

const mocks = vi.hoisted(() => {
  const state: { sub: Sub | null } = { sub: null };
  return {
    state,
    subscribe: vi.fn(async (_sql: string, _params: unknown[], options: Sub) => {
      state.sub = options;
      options.onData([]);
      return async () => {};
    }),
    embedAndIndexChunks: vi.fn(async (_sessionId: string, _chunks: unknown[]) => ({
      status: "ok",
      data: 0,
    })),
    deleteSessionChunks: vi.fn(async (_sessionId: string) => ({ status: "ok", data: 0 })),
    embeddingIndexStatus: vi.fn(async () => ({
      status: "ok",
      data: { modelDownloaded: true, chunkCount: 0 },
    })),
  };
});

vi.mock("~/db", () => ({ liveQueryClient: { subscribe: mocks.subscribe } }));
vi.mock("@hypr/plugin-embedding-search", () => ({
  commands: {
    embedAndIndexChunks: mocks.embedAndIndexChunks,
    deleteSessionChunks: mocks.deleteSessionChunks,
    embeddingIndexStatus: mocks.embeddingIndexStatus,
  },
}));
vi.mock("@hypr/editor/markdown", () => ({ json2md: (v: unknown) => String(v) }));

import { createSemanticIndexSync } from "./semantic-indexing";

const FAST = { drainDelayMs: 0, drainBatch: 5, modelRetryMs: 5 };

function noteRow(id: string, body: string) {
  return { id, raw_body: body, enhanced_notes_json: "[]", transcripts_json: "[]" };
}

function emit(rows: unknown[]) {
  mocks.state.sub!.onData(rows);
}

beforeEach(() => {
  mocks.state.sub = null;
  mocks.subscribe.mockClear();
  mocks.embedAndIndexChunks.mockClear();
  mocks.deleteSessionChunks.mockClear();
  mocks.embeddingIndexStatus.mockClear();
  mocks.embeddingIndexStatus.mockResolvedValue({
    status: "ok",
    data: { modelDownloaded: true, chunkCount: 0 },
  });
});

describe("semantic index sync", () => {
  it("indexes a session with content once, and NOT again when unchanged", async () => {
    const sync = createSemanticIndexSync(FAST);
    await sync.start();
    emit([noteRow("s1", "Discuss the Q3 roadmap and assign owners.")]);

    await vi.waitFor(() => expect(mocks.embedAndIndexChunks).toHaveBeenCalledTimes(1));
    expect(mocks.embedAndIndexChunks.mock.calls[0][0]).toBe("s1");

    // Re-emit identical content -> change detection skips it.
    emit([noteRow("s1", "Discuss the Q3 roadmap and assign owners.")]);
    await new Promise((r) => setTimeout(r, 10));
    expect(mocks.embedAndIndexChunks).toHaveBeenCalledTimes(1);
    await sync.stop();
  });

  it("re-indexes when a session's content changes", async () => {
    const sync = createSemanticIndexSync(FAST);
    await sync.start();
    emit([noteRow("s1", "first version of the note")]);
    await vi.waitFor(() => expect(mocks.embedAndIndexChunks).toHaveBeenCalledTimes(1));

    emit([noteRow("s1", "a completely different second version")]);
    await vi.waitFor(() => expect(mocks.embedAndIndexChunks).toHaveBeenCalledTimes(2));
    await sync.stop();
  });

  it("deletes chunks when a session is removed", async () => {
    const sync = createSemanticIndexSync(FAST);
    await sync.start();
    emit([noteRow("s1", "note that will be removed")]);
    await vi.waitFor(() => expect(mocks.embedAndIndexChunks).toHaveBeenCalledTimes(1));

    emit([]); // s1 gone
    await vi.waitFor(() => expect(mocks.deleteSessionChunks).toHaveBeenCalledWith("s1"));
    await sync.stop();
  });

  it("does NOT index while the model is not downloaded (no-op, no throw)", async () => {
    mocks.embeddingIndexStatus.mockResolvedValue({
      status: "ok",
      data: { modelDownloaded: false, chunkCount: 0 },
    });
    const sync = createSemanticIndexSync(FAST);
    await sync.start();
    emit([noteRow("s1", "content the model can't embed yet")]);
    await new Promise((r) => setTimeout(r, 30));
    expect(mocks.embedAndIndexChunks).not.toHaveBeenCalled();
    await sync.stop();
  });
});

import type { DatabaseSync } from "node:sqlite";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { createMigratedDb, queryAll } from "~/calendar/test-sqlite";

const state = vi.hoisted(() => ({
  db: null as unknown as DatabaseSync,
  embedding: vi.fn(),
}));

// The app's transaction semantics (plugins/db runtime.rs): all statements in
// one transaction, and a statement whose `expectedRowsAffected` does not match
// aborts and rolls back the whole batch.
vi.mock("~/db", () => ({
  executeTransaction: async (
    statements: Array<{
      sql: string;
      params?: unknown[];
      expectedRowsAffected?: number;
    }>,
  ) => {
    state.db.exec("BEGIN IMMEDIATE");
    try {
      const rows = statements.map((statement, index) => {
        const { changes } = state.db
          .prepare(statement.sql)
          .run(...((statement.params ?? []) as never[]));
        if (
          statement.expectedRowsAffected !== undefined &&
          Number(changes) !== statement.expectedRowsAffected
        ) {
          throw new Error(`statement ${index} affected ${changes} rows`);
        }
        return Number(changes);
      });
      state.db.exec("COMMIT");
      return rows;
    } catch (error) {
      state.db.exec("ROLLBACK");
      throw error;
    }
  },
  liveQueryClient: {
    execute: async (sql: string, params?: unknown[]) =>
      queryAll(state.db, sql, params),
  },
  useLiveQuery: vi.fn(),
}));

vi.mock("~/db/write-queue", () => ({
  enqueueDatabaseWrite: (_key: string, write: () => Promise<unknown>) =>
    write(),
}));

vi.mock("@hypr/plugin-transcription", () => ({
  commands: { computeVoiceEmbedding: state.embedding },
}));

import { deleteHuman, mergeHumans } from "./queries";
import {
  buildRecognizedSpeakerHints,
  collectVoiceSpans,
  decodeEmbeddingHex,
  encodeEmbeddingHex,
  enrollVoice,
  forgetVoiceProfile,
  loadEnrolledVoiceProfiles,
  selectSpeakerWords,
  VOICE_EMBEDDING_MODEL,
} from "./voice-profiles";

function addHuman(id: string, email = "") {
  state.db
    .prepare(`INSERT INTO humans (id, name, email) VALUES (?, ?, ?)`)
    .run(id, id, email);
}

async function enroll(humanId: string, embedding: number[]) {
  state.embedding.mockResolvedValueOnce({ status: "ok", data: embedding });
  await enrollVoice({ humanId, audioPath: "/rec.wav", spans: [] });
}

function storedProfiles() {
  return queryAll<{ human_id: string; embedding_hex: string; dim: number }>(
    state.db,
    `SELECT human_id, hex(embedding) AS embedding_hex, dim
     FROM voice_profiles ORDER BY human_id`,
  );
}

beforeEach(() => {
  state.db = createMigratedDb();
  state.embedding.mockReset();
});

describe("embedding hex encoding", () => {
  it("writes f32 little-endian, the layout the Rust side reads", () => {
    expect(encodeEmbeddingHex([1, -2])).toBe("0000803f000000c0");
  });

  it("rejects a blob that is not whole f32 values", () => {
    expect(() => decodeEmbeddingHex("")).toThrow();
    expect(() => decodeEmbeddingHex("0000803f00")).toThrow();
    expect(() => decodeEmbeddingHex("zz00803f")).toThrow();
  });
});

describe("voice profile storage (real schema)", () => {
  it("stores the embedding as a BLOB and reads the same floats back", async () => {
    addHuman("alice");
    await enroll("alice", [0.125, -0.5, 3.25]);

    const [row] = storedProfiles();
    expect(row.dim).toBe(3);
    expect(
      queryAll<{ t: string }>(
        state.db,
        `SELECT typeof(embedding) AS t FROM voice_profiles`,
      )[0].t,
    ).toBe("blob");
    await expect(loadEnrolledVoiceProfiles()).resolves.toEqual([
      { human_id: "alice", embedding: [0.125, -0.5, 3.25] },
    ]);
  });

  it("keeps one profile per person: enrolling again replaces it", async () => {
    addHuman("alice");
    await enroll("alice", [1, 2]);
    await enroll("alice", [3, 4]);

    expect(storedProfiles()).toHaveLength(1);
    await expect(loadEnrolledVoiceProfiles()).resolves.toEqual([
      { human_id: "alice", embedding: [3, 4] },
    ]);
  });

  it("refuses to save a voice for a deleted contact and leaves nothing behind", async () => {
    addHuman("alice");
    await deleteHuman("alice");

    await expect(enroll("alice", [1, 2])).rejects.toThrow();
    expect(storedProfiles()).toEqual([]);
  });

  it("deleting a contact deletes the voice outright", async () => {
    addHuman("alice");
    await enroll("alice", [1, 2]);
    await deleteHuman("alice");

    expect(storedProfiles()).toEqual([]);
  });

  it("forget removes the row, not just its visibility", async () => {
    addHuman("alice");
    await enroll("alice", [1, 2]);
    await forgetVoiceProfile("alice");

    expect(storedProfiles()).toEqual([]);
  });

  it("skips profiles from another embedding model", async () => {
    addHuman("bob");
    state.db
      .prepare(
        `INSERT INTO voice_profiles (id, human_id, embedding, dim, model)
         VALUES ('old', 'bob', unhex(?), 1, 'some-older-model')`,
      )
      .run(encodeEmbeddingHex([9]));

    await expect(loadEnrolledVoiceProfiles()).resolves.toEqual([]);
  });

  describe("merging contacts", () => {
    beforeEach(() => {
      addHuman("primary", "a@example.com");
      addHuman("duplicate", "a@example.com");
    });

    it("keeps the primary's voice and drops the duplicate's", async () => {
      await enroll("primary", [1]);
      await enroll("duplicate", [2]);
      await mergeHumans("primary", "duplicate");

      expect(storedProfiles()).toEqual([
        {
          human_id: "primary",
          embedding_hex: encodeEmbeddingHex([1]).toUpperCase(),
          dim: 1,
        },
      ]);
    });

    it("inherits the duplicate's voice when the primary has none usable", async () => {
      state.db
        .prepare(
          `INSERT INTO voice_profiles (id, human_id, embedding, dim, model)
           VALUES ('old', 'primary', unhex(?), 1, 'some-older-model')`,
        )
        .run(encodeEmbeddingHex([7]));
      await enroll("duplicate", [2]);
      await mergeHumans("primary", "duplicate");

      await expect(loadEnrolledVoiceProfiles()).resolves.toEqual([
        { human_id: "primary", embedding: [2] },
      ]);
      expect(
        queryAll(
          state.db,
          `SELECT 1 FROM voice_profiles WHERE human_id = 'duplicate'`,
        ),
      ).toEqual([]);
    });
  });

  it("tags rows with the bundled model", async () => {
    addHuman("alice");
    await enroll("alice", [1]);
    expect(
      queryAll<{ model: string }>(state.db, `SELECT model FROM voice_profiles`),
    ).toEqual([{ model: VOICE_EMBEDDING_MODEL }]);
  });
});

describe("collectVoiceSpans", () => {
  it("merges continuous speech and shifts it onto the recording timeline", () => {
    const { spans, speechMs } = collectVoiceSpans(
      [
        { start_ms: 0, end_ms: 400 },
        { start_ms: 600, end_ms: 1_000 },
        { start_ms: 2_000, end_ms: 2_500 },
      ],
      10_000,
    );
    expect(spans).toEqual([
      { start_ms: 10_000, end_ms: 11_000 },
      { start_ms: 12_000, end_ms: 12_500 },
    ]);
    expect(speechMs).toBe(1_500);
  });

  it("skips words without usable timing", () => {
    const { spans } = collectVoiceSpans(
      [
        { start_ms: null, end_ms: 400 },
        { start_ms: 500, end_ms: 500 },
        { start_ms: 900, end_ms: 800 },
        { start_ms: 1_000, end_ms: 1_200 },
      ],
      0,
    );
    expect(spans).toEqual([{ start_ms: 1_000, end_ms: 1_200 }]);
  });

  it("keeps the longest stretches up to a minute, in recording order", () => {
    const words = [
      { start_ms: 0, end_ms: 5_000 },
      { start_ms: 10_000, end_ms: 50_000 },
      { start_ms: 60_000, end_ms: 61_000 },
      { start_ms: 70_000, end_ms: 100_000 },
    ];
    const { spans, speechMs } = collectVoiceSpans(words, 0);
    expect(speechMs).toBe(60_000);
    expect(spans).toEqual([
      { start_ms: 10_000, end_ms: 50_000 },
      { start_ms: 70_000, end_ms: 90_000 },
    ]);
  });
});

describe("selectSpeakerWords", () => {
  const transcript = {
    words: [
      { id: "w1", start_ms: 0, end_ms: 1, channel: 1 },
      { id: "w2", start_ms: 2, end_ms: 3, channel: 1 },
      { id: "w3", start_ms: 4, end_ms: 5, channel: 0 },
    ],
    speakerHints: [
      {
        word_id: "w1",
        type: "provider_speaker_index",
        value: '{"speaker_index":2}',
      },
      {
        word_id: "w2",
        type: "provider_speaker_index",
        value: { speaker_index: 0 },
      },
      {
        word_id: "w3",
        type: "provider_speaker_index",
        value: '{"speaker_index":2}',
      },
    ],
  };
  const segmentWords = [{ id: "seg", start_ms: 9, end_ms: 10 }];

  it("takes every word of a diarized speaker on the segment's channel", () => {
    const words = selectSpeakerWords(
      transcript,
      {
        key: { channel: "RemoteParty", speaker_index: 2 },
        words: segmentWords,
      },
      "all",
    );
    expect(words.map((word) => word.id)).toEqual(["w1"]);
  });

  it("falls back to the segment's words without a speaker index or in segment mode", () => {
    expect(
      selectSpeakerWords(
        transcript,
        { key: { channel: "RemoteParty" }, words: segmentWords },
        "all",
      ),
    ).toBe(segmentWords);
    expect(
      selectSpeakerWords(
        transcript,
        {
          key: { channel: "RemoteParty", speaker_index: 2 },
          words: segmentWords,
        },
        "segment",
      ),
    ).toBe(segmentWords);
  });
});

describe("buildRecognizedSpeakerHints", () => {
  it("anchors each recognized speaker on its first word", () => {
    const hints = buildRecognizedSpeakerHints(
      [
        { word_id: "w1", speaker_index: 1 },
        { word_id: "w2", speaker_index: 0 },
        { word_id: "w3", speaker_index: 1 },
      ],
      [
        { speaker_index: 1, human_id: "alice" },
        { speaker_index: 0, human_id: "bob" },
        { speaker_index: 7, human_id: "carol" },
      ],
    );
    expect(hints).toEqual([
      {
        id: "w1:user_speaker_assignment:recognized",
        word_id: "w1",
        type: "user_speaker_assignment",
        value: JSON.stringify({ human_id: "alice" }),
      },
      {
        id: "w2:user_speaker_assignment:recognized",
        word_id: "w2",
        type: "user_speaker_assignment",
        value: JSON.stringify({ human_id: "bob" }),
      },
    ]);
  });
});

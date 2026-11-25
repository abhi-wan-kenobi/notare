import { beforeEach, describe, expect, it, vi } from "vitest";

import {
  buildRecognizedSpeakerHints,
  collectVoiceSpans,
  decodeEmbeddingHex,
  encodeEmbeddingHex,
  loadEnrolledVoiceProfiles,
} from "./voice-profiles";

const { executeMock } = vi.hoisted(() => ({ executeMock: vi.fn() }));

vi.mock("~/db", () => ({
  executeTransaction: vi.fn(),
  liveQueryClient: { execute: executeMock },
  useLiveQuery: vi.fn(),
}));

describe("embedding hex encoding", () => {
  it("writes f32 little-endian, the layout the Rust side reads", () => {
    expect(encodeEmbeddingHex([1, -2])).toBe("0000803f000000c0");
  });

  it("round-trips through the hex form SQLite's hex() returns", () => {
    const embedding = [0.125, -0.5, 3.25, 0];
    const hex = encodeEmbeddingHex(embedding).toUpperCase();
    expect(decodeEmbeddingHex(hex)).toEqual(embedding);
  });

  it("rejects a blob that is not whole f32 values", () => {
    expect(() => decodeEmbeddingHex("")).toThrow();
    expect(() => decodeEmbeddingHex("0000803f00")).toThrow();
    expect(() => decodeEmbeddingHex("zz00803f")).toThrow();
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
        id: "w1:user_speaker_assignment",
        word_id: "w1",
        type: "user_speaker_assignment",
        value: JSON.stringify({ human_id: "alice" }),
      },
      {
        id: "w2:user_speaker_assignment",
        word_id: "w2",
        type: "user_speaker_assignment",
        value: JSON.stringify({ human_id: "bob" }),
      },
    ]);
  });
});

describe("loadEnrolledVoiceProfiles", () => {
  beforeEach(() => {
    executeMock.mockReset();
  });

  it("keeps the newest readable profile per person", async () => {
    executeMock.mockResolvedValue([
      { human_id: "alice", embedding_hex: encodeEmbeddingHex([1, 2]) },
      { human_id: "alice", embedding_hex: encodeEmbeddingHex([9, 9]) },
      { human_id: "bob", embedding_hex: "00" },
      { human_id: "bob", embedding_hex: encodeEmbeddingHex([3]) },
    ]);
    const errors = vi.spyOn(console, "error").mockImplementation(() => {});

    await expect(loadEnrolledVoiceProfiles()).resolves.toEqual([
      { human_id: "alice", embedding: [1, 2] },
      { human_id: "bob", embedding: [3] },
    ]);
    expect(errors).toHaveBeenCalledTimes(1);
    errors.mockRestore();
  });
});

import { describe, expect, it } from "vitest";

import { buildChapters } from "./chapters";

describe("buildChapters", () => {
  it("matches headings to transcript text and derives chapter durations", () => {
    const chapters = buildChapters(
      "# Recap\n\n## Gateway token rotation\n\n## Speech engine benchmarks",
      [
        { text: "We start with the kickoff", start_ms: 0, end_ms: 2000 },
        {
          text: "Gateway tokens rotate every day",
          start_ms: 5000,
          end_ms: 8000,
        },
        {
          text: "Speech engine benchmarks show low latency",
          start_ms: 15000,
          end_ms: 19000,
        },
      ],
    );

    expect(chapters).toEqual([
      { title: "Gateway token rotation", startMs: 5000, durationMs: 10000 },
      { title: "Speech engine benchmarks", startMs: 15000, durationMs: 4000 },
    ]);
  });

  it("falls back to proportional transcript splits when no heading words match", () => {
    const chapters = buildChapters("## Alpha topic\n## Beta topic", [
      { text: "Opening", start_ms: 0, end_ms: 1000 },
      { text: "Middle", start_ms: 1000, end_ms: 2000 },
      { text: "Close", start_ms: 2000, end_ms: 3000 },
    ]);

    expect(chapters.map(({ startMs }) => startMs)).toEqual([0, 2000]);
  });
});

import { generateText, streamText } from "ai";
import { describe, expect, it } from "vitest";

import { embeddedLanguageModel } from "./index";

/**
 * The embedded llama.cpp server speaks OpenAI chat-completions SSE. Local
 * reasoning models (Qwen3, Gemma) put their scratchpad inline as
 * `<think>…</think>`, often split across chunks. Before 2026-11-04 the
 * embedded model was the one provider built without the extraction
 * middleware, and the scratchpad landed in meeting summaries.
 */
function sseFetch(deltas: string[]): typeof fetch {
  return async (_url, init) => {
    const body: unknown = JSON.parse(String(init?.body));
    const stream =
      typeof body === "object" &&
      body !== null &&
      "stream" in body &&
      body.stream === true;
    if (!stream) {
      return new Response(
        JSON.stringify({
          id: "c1",
          object: "chat.completion",
          created: 0,
          model: "qwen3-4b",
          choices: [
            {
              index: 0,
              message: { role: "assistant", content: deltas.join("") },
              finish_reason: "stop",
            },
          ],
        }),
        { status: 200, headers: { "content-type": "application/json" } },
      );
    }
    const lines = [
      ...deltas.map((content) =>
        JSON.stringify({
          id: "c1",
          object: "chat.completion.chunk",
          created: 0,
          model: "qwen3-4b",
          choices: [{ index: 0, delta: { content }, finish_reason: null }],
        }),
      ),
      JSON.stringify({
        id: "c1",
        object: "chat.completion.chunk",
        created: 0,
        model: "qwen3-4b",
        choices: [{ index: 0, delta: {}, finish_reason: "stop" }],
      }),
    ];
    const sse =
      lines.map((l) => `data: ${l}\n\n`).join("") + "data: [DONE]\n\n";
    return new Response(sse, {
      status: 200,
      headers: { "content-type": "text/event-stream" },
    });
  };
}

const QWEN_OUTPUT = [
  "<thi",
  "nk>\nThe user wants a summary. Let me list",
  " the decisions first.\n</th",
  "ink>\n\n## Summary\n",
  "- Ship 0.6.7 on Friday",
];

describe("embedded local model", () => {
  it("streams the summary without the model's <think> block (chunk-split tags)", async () => {
    const model = embeddedLanguageModel(
      "qwen3-4b",
      "http://127.0.0.1:9/v1",
      sseFetch(QWEN_OUTPUT),
    );
    const result = streamText({ model, prompt: "Summarize" });

    let text = "";
    let reasoning = "";
    for await (const part of result.fullStream) {
      if (part.type === "text-delta") text += part.text;
      if (part.type === "reasoning-delta") reasoning += part.text;
    }

    expect(text).not.toMatch(/think/i);
    expect(text).toContain("## Summary");
    expect(text).toContain("Ship 0.6.7 on Friday");
    expect(reasoning).toContain("Let me list the decisions first.");
  });

  it("drops <think> from non-streaming calls too (titles, structured output)", async () => {
    const model = embeddedLanguageModel(
      "qwen3-4b",
      "http://127.0.0.1:9/v1",
      sseFetch(["<think>pick a short title</think>Release planning"]),
    );
    const { text } = await generateText({ model, prompt: "Title" });
    expect(text.trim()).toBe("Release planning");
  });
});

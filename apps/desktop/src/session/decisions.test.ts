import { describe, expect, it } from "vitest";

import { extractDecisions } from "./decisions";

describe("extractDecisions", () => {
  it("removes a fenced decisions block and accepts only valid decision records", () => {
    const result = extractDecisions(
      '# Summary\n\nShip it.\n\n```decisions\n[{"text":"Ship by Friday","owner":"Mina","at_ms":4200},{"text":"No owner","owner":""},{"owner":"Mina"}]\n```\n',
    );

    expect(result).toEqual({
      markdown: "# Summary\n\nShip it.",
      decisions: [
        { text: "Ship by Friday", owner: "Mina", atMs: 4200 },
        { text: "No owner", owner: null, atMs: null },
      ],
    });
  });

  it("leaves ordinary markdown and malformed decision blocks untouched", () => {
    const markdown = "# Summary\n\n```decisions\nnot json\n```";
    expect(extractDecisions(markdown)).toEqual({ markdown, decisions: [] });
  });
});

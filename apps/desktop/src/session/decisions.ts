export type ExtractedDecision = {
  text: string;
  owner: string | null;
  atMs: number | null;
};

export function extractDecisions(markdown: string): {
  markdown: string;
  decisions: ExtractedDecision[];
} {
  const block = /```decisions\s*\n([\s\S]*?)\n```/i.exec(markdown);
  if (!block || block.index === undefined) {
    return { markdown, decisions: [] };
  }

  let parsed: unknown;
  try {
    parsed = JSON.parse(block[1]);
  } catch {
    return { markdown, decisions: [] };
  }
  if (!Array.isArray(parsed)) {
    return { markdown, decisions: [] };
  }

  const decisions = parsed.flatMap((entry): ExtractedDecision[] => {
    if (!entry || typeof entry !== "object" || Array.isArray(entry)) return [];
    const value = entry as { text?: unknown; owner?: unknown; at_ms?: unknown };
    if (typeof value.text !== "string" || !value.text.trim()) return [];
    return [
      {
        text: value.text.trim(),
        owner:
          typeof value.owner === "string" && value.owner.trim()
            ? value.owner.trim()
            : null,
        atMs:
          typeof value.at_ms === "number" &&
          Number.isFinite(value.at_ms) &&
          value.at_ms >= 0
            ? value.at_ms
            : null,
      },
    ];
  });

  return {
    markdown:
      `${markdown.slice(0, block.index)}${markdown.slice(block.index + block[0].length)}`.trim(),
    decisions,
  };
}

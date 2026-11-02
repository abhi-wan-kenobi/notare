export type ParsedCitation = {
  sessionId: string;
  ms: number;
  raw: string;
};

export type CitationSegment =
  | { type: "text"; text: string }
  | { type: "citation"; sessionId: string; ms: number; raw: string };

const CITATION_REGEX = /\[\[src:([a-zA-Z0-9_\-.:]+)@(\d+)\]\]/g;
const SINGLE_CITATION_REGEX = /^\[\[src:([a-zA-Z0-9_\-.:]+)@(\d+)\]\]$/;

/**
 * Validates and parses a single citation string.
 * Returns { sessionId, ms } if valid, otherwise null.
 */
export function parseCitation(
  raw: string,
): { sessionId: string; ms: number } | null {
  const match = SINGLE_CITATION_REGEX.exec(raw.trim());
  if (!match) {
    return null;
  }

  const [, sessionId, msStr] = match;
  if (!sessionId || sessionId.length === 0) {
    return null;
  }

  const ms = Number.parseInt(msStr, 10);
  if (Number.isNaN(ms) || ms < 0) {
    return null;
  }

  return { sessionId, ms };
}

/**
 * Extracts all valid citations from a block of text.
 */
export function extractCitations(text: string): ParsedCitation[] {
  const citations: ParsedCitation[] = [];
  const regex = new RegExp(CITATION_REGEX.source, "g");
  let match: RegExpExecArray | null;

  while ((match = regex.exec(text)) !== null) {
    const [raw, sessionId, msStr] = match;
    const ms = Number.parseInt(msStr, 10);
    if (!Number.isNaN(ms) && ms >= 0 && sessionId.length > 0) {
      citations.push({
        sessionId,
        ms,
        raw,
      });
    }
  }

  return citations;
}

/**
 * Parses text into an alternating sequence of plain text and citation segments,
 * handling adjacent text, multiple adjacent citations, and ignoring malformed tags.
 */
export function parseCitationSegments(text: string): CitationSegment[] {
  const segments: CitationSegment[] = [];
  const regex = new RegExp(CITATION_REGEX.source, "g");
  let lastIndex = 0;
  let match: RegExpExecArray | null;

  while ((match = regex.exec(text)) !== null) {
    const matchIndex = match.index;
    const [raw, sessionId, msStr] = match;
    const ms = Number.parseInt(msStr, 10);

    if (matchIndex > lastIndex) {
      segments.push({
        type: "text",
        text: text.slice(lastIndex, matchIndex),
      });
    }

    if (!Number.isNaN(ms) && ms >= 0 && sessionId.length > 0) {
      segments.push({
        type: "citation",
        sessionId,
        ms,
        raw,
      });
    } else {
      segments.push({
        type: "text",
        text: raw,
      });
    }

    lastIndex = matchIndex + raw.length;
  }

  if (lastIndex < text.length) {
    segments.push({
      type: "text",
      text: text.slice(lastIndex),
    });
  }

  return segments;
}

/**
 * Formats a millisecond timestamp into a human-readable string (e.g. 01:23 or 1:02:03).
 */
export function formatTimestamp(ms: number): string {
  const totalSeconds = Math.max(0, Math.floor(ms / 1000));
  const hours = Math.floor(totalSeconds / 3600);
  const minutes = Math.floor((totalSeconds % 3600) / 60);
  const seconds = totalSeconds % 60;
  const pad = (n: number) => String(n).padStart(2, "0");

  if (hours > 0) {
    return `${hours}:${pad(minutes)}:${pad(seconds)}`;
  }
  return `${pad(minutes)}:${pad(seconds)}`;
}

/**
 * Replaces citation tokens outside of code blocks with a markdown link that can be
 * intercepted and rendered by the chat message renderer as an interactive chip.
 */
export function replaceCitationsForRendering(text: string): string {
  // Split on code fences so we don't rewrite citations inside code blocks
  const parts = text.split(/(```[\s\S]*?```|`[^`]*`)/g);

  return parts
    .map((part) => {
      // If this part is a code fence or inline code, preserve verbatim
      if (part.startsWith("`")) {
        return part;
      }

      return part.replace(
        /\[\[src:([a-zA-Z0-9_\-.:]+)@(\d+)\]\]/g,
        (_match, sessionId, msStr) => {
          return `[src:${sessionId}@${msStr}](citation:${sessionId}@${msStr})`;
        },
      );
    })
    .join("");
}

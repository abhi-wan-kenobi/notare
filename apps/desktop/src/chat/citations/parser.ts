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
 * Parses timestamp string (mm:ss or hh:mm:ss) into milliseconds.
 */
export function parseTimestampToMs(timestamp: string): number | null {
  const parts = timestamp
    .trim()
    .split(":")
    .map((p) => Number.parseInt(p, 10));
  if (parts.some((p) => Number.isNaN(p) || p < 0)) return null;

  if (parts.length === 2) {
    const [minutes, seconds] = parts;
    if (seconds === undefined || minutes === undefined || seconds >= 60) {
      return null;
    }
    return (minutes * 60 + seconds) * 1000;
  }
  if (parts.length === 3) {
    const [hours, minutes, seconds] = parts;
    if (
      hours === undefined ||
      minutes === undefined ||
      seconds === undefined ||
      minutes >= 60 ||
      seconds >= 60
    ) {
      return null;
    }
    return (hours * 3600 + minutes * 60 + seconds) * 1000;
  }
  return null;
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

      let result = part.replace(
        /\[\[src:([a-zA-Z0-9_\-.:]+)@(\d+)\]\]/g,
        (_match, sessionId, msStr) => {
          return `[src:${sessionId}@${msStr}](citation:${sessionId}@${msStr})`;
        },
      );

      result = result.replace(
        /\[(\d{1,2}:\d{2}(?::\d{2})?)\](?!\()/g,
        (match, timeStr) => {
          const ms = parseTimestampToMs(timeStr);
          if (ms === null) return match;
          return `[▸ ${timeStr}](citation-timestamp:${ms})`;
        },
      );

      result = result.replace(
        /(?:"at_ms"|at_ms)\s*[:=]\s*(\d+)/g,
        (match, msStr) => {
          const ms = Number.parseInt(msStr, 10);
          if (Number.isNaN(ms) || ms < 0) return match;
          const formatted = formatTimestamp(ms);
          return `at_ms: [▸ ${formatted}](citation-timestamp:${ms})`;
        },
      );

      return result;
    })
    .join("");
}

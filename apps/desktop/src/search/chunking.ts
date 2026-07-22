/**
 * Chunking for the semantic index (WS-B2 indexer core).
 *
 * Turns a session's note + transcript into ~200-token, sentence-ish chunks with
 * a stable content hash and (for transcript chunks) the start_ms of their first
 * word — so a semantic hit can jump-to-source via seekAndPlay. Pure + testable;
 * the reconcile/idle-backfill wiring calls these and hands the result to the
 * `embed_and_index_chunks` plugin command.
 */

export type ChunkSourceType = "note" | "transcript";

/** Matches the embedding-search plugin's `ChunkInput` binding. */
export type ChunkInput = {
  text: string;
  source_type: ChunkSourceType;
  start_ms: number | null;
  content_hash: string;
};

export type ChunkWord = { text: string; start_ms: number };

// ~200 tokens; a token averages ~4 chars of English, so ~800 chars is a good
// proxy without a real tokenizer on the hot path.
const TARGET_CHARS = 800;
// Don't emit a trailing sliver as its own chunk; fold anything shorter than
// this into the previous chunk.
const MIN_TAIL_CHARS = 120;

/** Fast, stable, synchronous content hash (FNV-1a, 32-bit, hex). */
export function contentHash(s: string): string {
  let h = 0x811c9dc5;
  for (let i = 0; i < s.length; i++) {
    h ^= s.charCodeAt(i);
    h = Math.imul(h, 0x01000193);
  }
  return (h >>> 0).toString(16).padStart(8, "0");
}

/** Split text into sentence-ish spans, keeping the delimiter with the sentence. */
function splitSentences(text: string): string[] {
  const parts = text
    .replace(/\s+/g, " ")
    .trim()
    .match(/[^.!?\n]+[.!?]*\s*/g);
  return (parts ?? []).map((s) => s.trim()).filter(Boolean);
}

/** Greedily pack sentences into ~TARGET_CHARS chunks. */
export function chunkNoteText(
  text: string,
  sourceType: ChunkSourceType = "note",
): ChunkInput[] {
  const sentences = splitSentences(text);
  const chunks: ChunkInput[] = [];
  let buf = "";

  const flush = () => {
    const t = buf.trim();
    if (t) {
      chunks.push({
        text: t,
        source_type: sourceType,
        start_ms: null,
        content_hash: contentHash(t),
      });
    }
    buf = "";
  };

  for (const sentence of sentences) {
    if (buf && buf.length + 1 + sentence.length > TARGET_CHARS) {
      flush();
    }
    buf = buf ? `${buf} ${sentence}` : sentence;
  }
  // Fold a short tail into the previous chunk instead of emitting a sliver.
  if (buf.trim().length < MIN_TAIL_CHARS && chunks.length > 0) {
    const last = chunks[chunks.length - 1];
    const merged = `${last.text} ${buf.trim()}`.trim();
    chunks[chunks.length - 1] = {
      ...last,
      text: merged,
      content_hash: contentHash(merged),
    };
    buf = "";
  } else {
    flush();
  }

  return chunks;
}

/**
 * Chunk a transcript's words into ~TARGET_CHARS chunks, each carrying the
 * start_ms of its first word.
 */
export function chunkTranscriptWords(words: ChunkWord[]): ChunkInput[] {
  const chunks: ChunkInput[] = [];
  let bufWords: string[] = [];
  let bufChars = 0;
  let startMs: number | null = null;

  const flush = () => {
    const t = bufWords.join(" ").trim();
    if (t) {
      chunks.push({
        text: t,
        source_type: "transcript",
        start_ms: startMs,
        content_hash: contentHash(t),
      });
    }
    bufWords = [];
    bufChars = 0;
    startMs = null;
  };

  for (const w of words) {
    const word = w.text.trim();
    if (!word) continue;
    if (startMs === null) startMs = w.start_ms;
    bufWords.push(word);
    bufChars += word.length + 1;
    if (bufChars >= TARGET_CHARS) flush();
  }
  flush();

  return chunks;
}

export type SessionChunkInput = {
  rawMarkdown?: string;
  enhancedMarkdown?: string[];
  transcriptWords?: ChunkWord[];
};

/** All chunks for a session (note + enhanced notes + transcript), de-duplicated
 * by content_hash so identical text isn't embedded twice. */
export function chunkSession(input: SessionChunkInput): ChunkInput[] {
  const all: ChunkInput[] = [
    ...chunkNoteText(input.rawMarkdown ?? ""),
    ...(input.enhancedMarkdown ?? []).flatMap((md) => chunkNoteText(md)),
    ...chunkTranscriptWords(input.transcriptWords ?? []),
  ];
  const seen = new Set<string>();
  return all.filter((c) => {
    const key = `${c.source_type}:${c.content_hash}`;
    if (seen.has(key)) return false;
    seen.add(key);
    return true;
  });
}

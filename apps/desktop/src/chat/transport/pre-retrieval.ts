import { commands as embeddingSearch } from "@hypr/plugin-embedding-search";
import { commands as tantivy } from "@hypr/plugin-tantivy";

import type { ToolScopeFilter } from "~/chat/state/chat-scope";
import { resolveAllowedSessionIds } from "~/chat/tools/scope-resolver";
import {
  hybridBySession,
  type LexicalHit,
  type SemanticHit,
} from "~/search/hybrid";

export type RetrievedChunk = {
  sessionId: string;
  startMs: number;
  text: string;
  sourceId: string;
  meetingTitle?: string;
};

export const CITATION_SYSTEM_GUIDANCE = `
Citation guidance:
- When using information from meeting notes, transcripts, or retrieved context, cite the source using the exact citation format: \`[[src:<sessionId>@<startMs>]]\`.
- Never invent or fabricate session IDs or timestamps. Only cite using the exact \`[[src:<sessionId>@<startMs>]]\` identifiers provided in context or tool results.
`.trim();

export function appendCitationGuidance(
  prompt: string | undefined,
): string | undefined {
  if (prompt === undefined) {
    return CITATION_SYSTEM_GUIDANCE;
  }
  if (!prompt.trim()) {
    return CITATION_SYSTEM_GUIDANCE;
  }
  return `${prompt.trim()}\n\n${CITATION_SYSTEM_GUIDANCE}`;
}

export function formatPreRetrievalBlock(chunks: RetrievedChunk[]): string {
  if (chunks.length === 0) {
    return "";
  }

  const items = chunks.map((chunk) => {
    const titleHeader = chunk.meetingTitle
      ? ` (Meeting: "${chunk.meetingTitle}")`
      : "";
    return `[Source ${chunk.sourceId}]${titleHeader}\n${chunk.text.trim()}`;
  });

  return [
    "<retrieved_meeting_context>",
    "The following excerpts were retrieved from relevant meetings. Cite facts from these excerpts using their exact citation IDs [[src:<sessionId>@<startMs>]].",
    "",
    items.join("\n\n"),
    "</retrieved_meeting_context>",
  ].join("\n");
}

export interface PreRetrievalOptions {
  query: string;
  scopeFilter?: ToolScopeFilter;
  limit?: number;
  lexicalSearch?: (query: string) => Promise<LexicalHit[]>;
  semanticSearch?: (query: string, k: number) => Promise<SemanticHit[]>;
  resolveAllowedIds?: (
    filter: ToolScopeFilter | undefined,
  ) => Promise<string[] | null>;
  sessionTitles?: Record<string, string>;
}

export async function runPreRetrieval({
  query,
  scopeFilter,
  limit = 8,
  lexicalSearch,
  semanticSearch,
  resolveAllowedIds,
  sessionTitles = {},
}: PreRetrievalOptions): Promise<RetrievedChunk[]> {
  const trimmed = query.trim();
  if (!trimmed) {
    return [];
  }

  const allowedSessionIds = resolveAllowedIds
    ? await resolveAllowedIds(scopeFilter)
    : await resolveAllowedSessionIds(scopeFilter);

  if (allowedSessionIds !== null && allowedSessionIds.length === 0) {
    return [];
  }

  const performLexical =
    lexicalSearch ??
    (async (q: string) => {
      try {
        const res = await tantivy.search({
          query: q,
          filters: { created_at: null, doc_type: "session", facet: null },
          limit: 20,
        });
        if (res.status === "ok") {
          return res.data.hits.map((h) => ({
            id: h.document.id,
            entityType: h.document.doc_type,
            title: h.document.title,
            content: h.document.content,
            score: h.score,
          }));
        }
      } catch (e) {
        console.error("Pre-retrieval lexical search failed", e);
      }
      return [];
    });

  const performSemantic =
    semanticSearch ??
    (async (q: string, k: number) => {
      try {
        const res = await embeddingSearch.semanticSearch(q, k, null);
        if (res.status === "ok") {
          return res.data.map((h) => ({
            chunkId: h.chunkId,
            sessionId: h.sessionId,
            sourceType: h.sourceType,
            text: h.text,
            startMs: h.startMs,
            distance: h.distance,
          }));
        }
      } catch (e) {
        console.error("Pre-retrieval dense search failed", e);
      }
      return [];
    });

  const [lexicalHits, semanticHits] = await Promise.all([
    performLexical(trimmed).catch(() => [] as LexicalHit[]),
    performSemantic(trimmed, 60).catch(() => [] as SemanticHit[]),
  ]);

  let fused = hybridBySession(lexicalHits, semanticHits);

  if (allowedSessionIds !== null) {
    fused = fused.filter((item) => allowedSessionIds.includes(item.sessionId));
  }

  const chunks: RetrievedChunk[] = [];

  for (const session of fused) {
    const title =
      session.lexical?.title ?? sessionTitles[session.sessionId] ?? undefined;

    if (session.semantic.length > 0) {
      for (const sem of session.semantic) {
        const startMs = sem.startMs ?? 0;
        chunks.push({
          sessionId: session.sessionId,
          startMs,
          text: sem.text,
          sourceId: `[[src:${session.sessionId}@${startMs}]]`,
          meetingTitle: title,
        });

        if (chunks.length >= limit) {
          break;
        }
      }
    } else if (session.lexical?.content) {
      chunks.push({
        sessionId: session.sessionId,
        startMs: 0,
        text: session.lexical.content.slice(0, 500),
        sourceId: `[[src:${session.sessionId}@0]]`,
        meetingTitle: title,
      });
    }

    if (chunks.length >= limit) {
      break;
    }
  }

  return chunks.slice(0, limit);
}

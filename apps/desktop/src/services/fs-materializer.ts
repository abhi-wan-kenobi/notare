import {
  commands as fsSyncCommands,
  type JsonValue,
  type ParsedDocument,
} from "@hypr/plugin-fs-sync";

import { liveQueryClient } from "~/db";
import {
  loadSessionContentSnapshot,
  type SessionContentSnapshot,
} from "~/session/content-queries";
import { useMountEffect } from "~/shared/hooks/useMountEffect";
import {
  buildRenderTranscriptRequestFromRows,
  renderTranscriptSegments,
} from "~/stt/render-transcript";

// Materializes sessions from the canonical SQLite database into the notes
// folder (vault) as plain files, so every session is readable outside the app:
//
//   sessions/<session-id>/
//     _meta.json        session metadata
//     _memo.md          the raw note (frontmatter + markdown)
//     <note-id>.md      each enhanced note (frontmatter + markdown)
//     transcript.json   the full transcript data
//     transcript.md     a human-readable transcript
//
// The TinyBase -> SQLite migration (upstream #5972) removed the file
// persisters that used to write these, leaving only the audio recording on
// disk. This service restores the DB -> disk direction of fs-sync.

export const SESSION_MATERIALIZE_DEBOUNCE_MS = 2_000;

const MATERIALIZED_STATE_STORAGE_KEY = "notare.fs-materializer.v1";

export const SESSION_DIRTY_SQL = `
  SELECT
    session.id AS id,
    MAX(
      session.updated_at,
      COALESCE((
        SELECT MAX(document.updated_at)
        FROM session_documents AS document
        WHERE document.session_id = session.id
      ), ''),
      COALESCE((
        SELECT MAX(transcript.updated_at)
        FROM transcripts AS transcript
        WHERE transcript.session_id = session.id
      ), ''),
      COALESCE((
        SELECT MAX(participant.updated_at)
        FROM session_participants AS participant
        WHERE participant.session_id = session.id
      ), '')
    ) AS dirty_key
  FROM sessions AS session
  WHERE session.deleted_at IS NULL
`;

export type SessionDirtyRow = {
  id: string;
  dirty_key: string;
};

export type SessionFilePayloads = {
  documents: Array<[ParsedDocument, string]>;
  json: Array<[JsonValue, string]>;
};

export type TranscriptMarkdownSegment = {
  speaker: string | null;
  startMs: number | null;
  text: string;
};

export function buildSessionFiles(
  snapshot: SessionContentSnapshot,
  transcriptMarkdown: string | null,
): SessionFilePayloads {
  const documents: Array<[ParsedDocument, string]> = [];
  const json: Array<[JsonValue, string]> = [];

  json.push([
    {
      id: snapshot.sessionId,
      userId: snapshot.ownerUserId,
      createdAt: snapshot.createdAt || null,
      title: snapshot.title || null,
      event: (snapshot.event ?? null) as JsonValue,
      eventId: snapshot.eventId,
    },
    "_meta.json",
  ]);

  if (snapshot.rawNoteId || snapshot.rawMarkdown.trim()) {
    documents.push([
      {
        frontmatter: {
          id: snapshot.rawNoteId ?? snapshot.sessionId,
          session_id: snapshot.sessionId,
          ...(snapshot.title ? { title: snapshot.title } : {}),
        },
        content: snapshot.rawMarkdown,
      },
      "_memo.md",
    ]);
  }

  for (const note of snapshot.enhancedNotes) {
    if (!note.id) {
      continue;
    }

    documents.push([
      {
        frontmatter: {
          id: note.id,
          session_id: snapshot.sessionId,
          ...(note.templateId ? { template_id: note.templateId } : {}),
          ...(Number.isFinite(note.position)
            ? { position: note.position }
            : {}),
          ...(note.title ? { title: note.title } : {}),
        },
        content: note.markdown,
      },
      `${note.id}.md`,
    ]);
  }

  if (snapshot.transcripts.length > 0) {
    json.push([
      {
        transcripts: snapshot.transcripts.map((transcript) => ({
          id: transcript.id,
          user_id: snapshot.ownerUserId,
          created_at: snapshot.createdAt,
          session_id: snapshot.sessionId,
          started_at: transcript.started_at,
          ended_at: transcript.ended_at,
          memo_md: transcript.memo ?? "",
          words: transcript.words,
          speaker_hints: transcript.speaker_hints,
        })),
      } as JsonValue,
      "transcript.json",
    ]);
  }

  if (transcriptMarkdown) {
    // No frontmatter on purpose: session-content readers identify note files
    // by their `session_id` frontmatter, so this file stays a plain export.
    documents.push([
      { frontmatter: {}, content: transcriptMarkdown },
      "transcript.md",
    ]);
  }

  return { documents, json };
}

export function formatTimestamp(ms: number): string {
  const totalSeconds = Math.max(0, Math.floor(ms / 1000));
  const hours = Math.floor(totalSeconds / 3600);
  const minutes = Math.floor((totalSeconds % 3600) / 60);
  const seconds = totalSeconds % 60;
  const mmss = `${String(minutes).padStart(2, "0")}:${String(seconds).padStart(2, "0")}`;
  return hours > 0 ? `${hours}:${mmss}` : mmss;
}

export function formatTranscriptMarkdown(
  segments: TranscriptMarkdownSegment[],
): string | null {
  const blocks: string[] = [];

  for (const segment of segments) {
    const text = segment.text.trim();
    if (!text) {
      continue;
    }

    const speaker = segment.speaker?.trim() || "Speaker";
    const timestamp =
      segment.startMs == null ? "" : ` [${formatTimestamp(segment.startMs)}]`;
    blocks.push(`**${speaker}**${timestamp}\n${text}`);
  }

  if (blocks.length === 0) {
    return null;
  }

  return `# Transcript\n\n${blocks.join("\n\n")}\n`;
}

export function buildFallbackTranscriptSegments(
  snapshot: SessionContentSnapshot,
): TranscriptMarkdownSegment[] {
  const segments: TranscriptMarkdownSegment[] = [];

  for (const transcript of snapshot.transcripts) {
    let current: TranscriptMarkdownSegment | null = null;

    for (const word of transcript.words) {
      if (typeof word.text !== "string" || !word.text) {
        continue;
      }

      const speaker = word.speaker ?? null;
      if (!current || current.speaker !== speaker) {
        current = {
          speaker,
          startMs: typeof word.start_ms === "number" ? word.start_ms : null,
          text: word.text,
        };
        segments.push(current);
      } else {
        current.text += ` ${word.text}`;
      }
    }
  }

  return segments;
}

async function renderSnapshotTranscriptMarkdown(
  snapshot: SessionContentSnapshot,
): Promise<string | null> {
  const hasWords = snapshot.transcripts.some(
    (transcript) => transcript.words.length > 0,
  );
  if (!hasWords) {
    return null;
  }

  const request = buildRenderTranscriptRequestFromRows(
    snapshot.transcripts.map((transcript) => ({
      started_at: transcript.started_at,
      words: transcript.words,
      speaker_hints: transcript.speaker_hints,
    })),
    {
      humans: snapshot.participants.map((participant) => ({
        human_id: participant.humanId,
        name: participant.name,
      })),
      selfHumanId: snapshot.ownerUserId || undefined,
    },
    snapshot.participants.map((participant) => participant.humanId),
  );

  if (request) {
    try {
      const segments = await renderTranscriptSegments(request);
      const markdown = formatTranscriptMarkdown(
        segments.map((segment) => ({
          speaker: segment.speaker_label,
          startMs: segment.start_ms,
          text: segment.text,
        })),
      );
      if (markdown) {
        return markdown;
      }
    } catch (error) {
      console.error(
        "[fs-materializer] transcript render failed, using fallback",
        error,
      );
    }
  }

  return formatTranscriptMarkdown(buildFallbackTranscriptSegments(snapshot));
}

function joinSessionPath(sessionDir: string, name: string): string {
  return sessionDir.endsWith("/") || sessionDir.endsWith("\\")
    ? `${sessionDir}${name}`
    : `${sessionDir}/${name}`;
}

export async function materializeSession(sessionId: string): Promise<boolean> {
  const snapshot = await loadSessionContentSnapshot(sessionId);
  if (!snapshot) {
    return false;
  }

  const dirResult = await fsSyncCommands.sessionDir(sessionId);
  if (dirResult.status === "error") {
    throw new Error(dirResult.error);
  }
  const sessionDir = dirResult.data;

  const transcriptMarkdown = await renderSnapshotTranscriptMarkdown(snapshot);
  const files = buildSessionFiles(snapshot, transcriptMarkdown);

  if (files.json.length > 0) {
    const result = await fsSyncCommands.writeJsonBatch(
      files.json.map(
        ([value, name]) =>
          [value, joinSessionPath(sessionDir, name)] as [JsonValue, string],
      ),
    );
    if (result.status === "error") {
      throw new Error(result.error);
    }
  }

  if (files.documents.length > 0) {
    const result = await fsSyncCommands.writeDocumentBatch(
      files.documents.map(
        ([doc, name]) =>
          [doc, joinSessionPath(sessionDir, name)] as [ParsedDocument, string],
      ),
    );
    if (result.status === "error") {
      throw new Error(result.error);
    }
  }

  return true;
}

export function collectChangedSessions(
  materialized: Record<string, string>,
  rows: SessionDirtyRow[],
): { changed: SessionDirtyRow[]; removedIds: string[] } {
  const seen = new Set<string>();
  const changed: SessionDirtyRow[] = [];

  for (const row of rows) {
    if (!row.id) {
      continue;
    }
    seen.add(row.id);
    if (materialized[row.id] !== row.dirty_key) {
      changed.push(row);
    }
  }

  const removedIds = Object.keys(materialized).filter((id) => !seen.has(id));

  return { changed, removedIds };
}

function loadMaterializedState(): Record<string, string> {
  try {
    const raw = globalThis.localStorage?.getItem(
      MATERIALIZED_STATE_STORAGE_KEY,
    );
    if (!raw) {
      return {};
    }
    const parsed = JSON.parse(raw) as unknown;
    if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
      return {};
    }
    return Object.fromEntries(
      Object.entries(parsed as Record<string, unknown>).filter(
        (entry): entry is [string, string] => typeof entry[1] === "string",
      ),
    );
  } catch {
    return {};
  }
}

function saveMaterializedState(state: Record<string, string>) {
  try {
    globalThis.localStorage?.setItem(
      MATERIALIZED_STATE_STORAGE_KEY,
      JSON.stringify(state),
    );
  } catch {
    // Best-effort: worst case sessions are re-materialized on next launch.
  }
}

export function SessionFsMaterializer() {
  useMountEffect(() => {
    let cancelled = false;
    let timeout: ReturnType<typeof setTimeout> | null = null;
    let unsubscribe: (() => Promise<void>) | null = null;
    let flushing = false;
    let rerunRequested = false;
    let latestRows: SessionDirtyRow[] = [];
    const materialized = loadMaterializedState();

    const flush = async () => {
      if (flushing) {
        rerunRequested = true;
        return;
      }
      flushing = true;

      try {
        const { changed, removedIds } = collectChangedSessions(
          materialized,
          latestRows,
        );

        let dirty = removedIds.length > 0;
        for (const id of removedIds) {
          delete materialized[id];
        }

        for (const row of changed) {
          if (cancelled) {
            break;
          }
          try {
            await materializeSession(row.id);
            materialized[row.id] = row.dirty_key;
            dirty = true;
          } catch (error) {
            console.error(
              `[fs-materializer] failed to write session ${row.id}`,
              error,
            );
          }
        }

        if (dirty) {
          saveMaterializedState(materialized);
        }
      } finally {
        flushing = false;
        if (rerunRequested && !cancelled) {
          rerunRequested = false;
          schedule();
        }
      }
    };

    const schedule = () => {
      if (timeout) {
        clearTimeout(timeout);
      }
      timeout = setTimeout(() => {
        timeout = null;
        void flush();
      }, SESSION_MATERIALIZE_DEBOUNCE_MS);
    };

    void liveQueryClient
      .subscribe<SessionDirtyRow>(SESSION_DIRTY_SQL, [], {
        onData: (rows) => {
          latestRows = rows;
          schedule();
        },
        onError: (error) => {
          console.error(
            "[fs-materializer] session change subscription failed",
            error,
          );
        },
      })
      .then((unsub) => {
        if (cancelled) {
          void unsub();
        } else {
          unsubscribe = unsub;
        }
      })
      .catch((error) => {
        console.error("[fs-materializer] failed to subscribe", error);
      });

    return () => {
      cancelled = true;
      if (timeout) {
        clearTimeout(timeout);
      }
      void unsubscribe?.();
    };
  });

  return null;
}

import type { DictationOutputMode } from "@hypr/plugin-dictation";

import { executeTransaction, liveQueryClient, useLiveQuery } from "~/db";
import { enqueueDatabaseWrite } from "~/db/write-queue";

/**
 * Dictation history - the searchable "in-app clipboard". Every completed
 * dictation is persisted (cleaned text + the pre-cleanup raw transcript,
 * source, model, duration, pin/status flags, timestamp) so it can be searched,
 * pinned and re-copied later from the History surface.
 *
 * Persistence choice: the app's SQLite DB (`dictation_history` table,
 * migrations `20260716120000_dictation_history` +
 * `20260731000000_dictation_history_snippets`), mirroring how every other list
 * in the app persists (chat groups/messages pattern: `useLiveQuery` reads +
 * `enqueueDatabaseWrite`-serialized transactions; one-shot reads via
 * `liveQueryClient.execute`, same as `contacts/queries`).
 */

/** Recent delivered entries shown in the legacy Settings history list. */
export const DICTATION_HISTORY_CAP = 50;

/**
 * Rolling retention cap. Pruning keeps the newest `DICTATION_HISTORY_PRUNE_CAP`
 * *unpinned* rows and never touches pinned ones, so a fully-pinned table is a
 * no-op rather than a deadlock.
 */
export const DICTATION_HISTORY_PRUNE_CAP = 500;

/** Default page size for {@link listDictationHistory}. */
export const DICTATION_HISTORY_PAGE_SIZE = 50;

const WRITE_QUEUE_KEY = "dictation-history";

export type DictationHistorySource = "dictation" | "meeting";
export type DictationHistoryStatus = "delivered" | "discarded";

export interface DictationHistoryEntry {
  id: string;
  /** Cleaned/delivered text. */
  text: string;
  /** Pre-cleanup raw transcript, when captured. */
  rawText: string | null;
  source: DictationHistorySource;
  model: string | null;
  durationMs: number | null;
  pinned: boolean;
  status: DictationHistoryStatus;
  /** ISO-8601 UTC timestamp. */
  createdAt: string;
}

type DictationHistorySqlRow = {
  id: string;
  text: string;
  raw_text: string | null;
  source: string;
  model: string | null;
  duration_ms: number | null;
  pinned: number;
  status: string;
  created_at: string;
};

const EMPTY_HISTORY: DictationHistoryEntry[] = [];

const SELECT_COLUMNS = `
  id, text, raw_text, source, model, duration_ms, pinned, status, created_at
`;

/**
 * Legacy Settings history list: the most recent *delivered* entries. Discarded
 * (recovery-only) rows are excluded so the copy list stays clean.
 */
export function useDictationHistory(): DictationHistoryEntry[] {
  const { data = EMPTY_HISTORY } = useLiveQuery<
    DictationHistorySqlRow,
    DictationHistoryEntry[]
  >({
    sql: `
      SELECT ${SELECT_COLUMNS}
      FROM dictation_history
      WHERE status = 'delivered'
      ORDER BY created_at DESC, id DESC
      LIMIT ?
    `,
    params: [DICTATION_HISTORY_CAP],
    mapRows: (rows) => rows.map(mapHistoryRow),
  });

  return data;
}

function mapHistoryRow(row: DictationHistorySqlRow): DictationHistoryEntry {
  return {
    id: row.id,
    text: row.text,
    rawText: row.raw_text ?? null,
    source: row.source === "meeting" ? "meeting" : "dictation",
    model: row.model ?? null,
    durationMs: row.duration_ms ?? null,
    pinned: row.pinned !== 0,
    status: row.status === "discarded" ? "discarded" : "delivered",
    createdAt: row.created_at,
  };
}

/**
 * Opaque keyset cursor over (created_at, id). Base64 keeps it treatable as a
 * blob by callers and out of the query surface. Round-tripped through UTF-8
 * bytes because btoa/atob alone are Latin-1-only - today's values are ASCII
 * (ISO timestamp + UUID), but the cursor primitive must not corrupt or throw
 * the day an id ever carries non-Latin-1 text.
 */
function encodeCursor(row: { createdAt: string; id: string }): string {
  const bytes = new TextEncoder().encode(JSON.stringify([row.createdAt, row.id]));
  return btoa(String.fromCharCode(...bytes));
}

function decodeCursor(cursor: string): { createdAt: string; id: string } | null {
  try {
    const bytes = Uint8Array.from(atob(cursor), (c) => c.charCodeAt(0));
    const parsed = JSON.parse(new TextDecoder().decode(bytes));
    if (
      Array.isArray(parsed) &&
      typeof parsed[0] === "string" &&
      typeof parsed[1] === "string"
    ) {
      return { createdAt: parsed[0], id: parsed[1] };
    }
  } catch {
    // Fall through: a malformed cursor just restarts from the first page.
  }
  return null;
}

/**
 * Turn free-text into a safe FTS5 MATCH expression: each whitespace token
 * becomes a quoted prefix term (implicit AND), so user input with FTS operator
 * characters (`"`, `*`, `:`, `AND`, `-`) can't throw a syntax error.
 */
function toFtsQuery(query: string): string {
  return query
    .trim()
    .split(/\s+/)
    .filter(Boolean)
    .map((token) => `"${token.replace(/"/g, '""')}"*`)
    .join(" ");
}

/**
 * Newest-first page of history, optionally full-text filtered over both the
 * cleaned text and the raw transcript. Keyset pagination on (created_at, id) -
 * no OFFSET, so deep pages stay cheap.
 */
export async function listDictationHistory(opts?: {
  query?: string;
  cursor?: string | null;
  limit?: number;
}): Promise<{ entries: DictationHistoryEntry[]; nextCursor: string | null }> {
  const limit =
    opts?.limit && opts.limit > 0 ? opts.limit : DICTATION_HISTORY_PAGE_SIZE;
  const query = opts?.query?.trim() ?? "";
  const after = opts?.cursor ? decodeCursor(opts.cursor) : null;

  const where: string[] = [];
  const params: unknown[] = [];

  if (query) {
    where.push(`dictation_history_fts MATCH ?`);
    params.push(toFtsQuery(query));
  }
  if (after) {
    where.push(
      `(h.created_at < ? OR (h.created_at = ? AND h.id < ?))`,
    );
    params.push(after.createdAt, after.createdAt, after.id);
  }

  const whereClause = where.length ? `WHERE ${where.join(" AND ")}` : "";
  // Fetch one extra row to know whether a further page exists.
  params.push(limit + 1);

  const sql = query
    ? `
        SELECT ${SELECT_COLUMNS.replace(/(\w+)/g, "h.$1")}
        FROM dictation_history h
        JOIN dictation_history_fts f ON f.id = h.id
        ${whereClause}
        ORDER BY h.created_at DESC, h.id DESC
        LIMIT ?
      `
    : `
        SELECT ${SELECT_COLUMNS.replace(/(\w+)/g, "h.$1")}
        FROM dictation_history h
        ${whereClause}
        ORDER BY h.created_at DESC, h.id DESC
        LIMIT ?
      `;

  const rows = await liveQueryClient.execute<DictationHistorySqlRow>(
    sql,
    params,
  );

  const hasMore = rows.length > limit;
  const page = hasMore ? rows.slice(0, limit) : rows;
  const entries = page.map(mapHistoryRow);
  const last = entries[entries.length - 1];
  const nextCursor = hasMore && last ? encodeCursor(last) : null;

  return { entries, nextCursor };
}

/**
 * Append a completed dictation and prune the oldest *unpinned* rows past the
 * rolling cap in the same transaction. New fields are optional so existing
 * callers passing only `{ text, mode, cleaned }` keep working.
 */
export async function addDictationHistoryEntry(entry: {
  text: string;
  mode: DictationOutputMode;
  cleaned: boolean;
  rawText?: string | null;
  source?: DictationHistorySource;
  model?: string | null;
  durationMs?: number | null;
  status?: DictationHistoryStatus;
}): Promise<void> {
  const id = crypto.randomUUID();
  const createdAt = new Date().toISOString();

  await enqueueDatabaseWrite(WRITE_QUEUE_KEY, async () => {
    await executeTransaction([
      {
        sql: `
          INSERT INTO dictation_history
            (id, text, raw_text, mode, cleaned, source, model, duration_ms,
             status, created_at)
          VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        `,
        params: [
          id,
          entry.text,
          entry.rawText ?? null,
          entry.mode,
          entry.cleaned ? 1 : 0,
          entry.source ?? "dictation",
          entry.model ?? null,
          entry.durationMs ?? null,
          entry.status ?? "delivered",
          createdAt,
        ],
      },
      {
        // Prune oldest-first among *unpinned* rows only; pinned rows are exempt
        // and never counted, so an all-pinned table prunes nothing.
        sql: `
          DELETE FROM dictation_history
          WHERE pinned = 0
            AND id NOT IN (
              SELECT id FROM dictation_history
              WHERE pinned = 0
              ORDER BY created_at DESC, id DESC
              LIMIT ?
            )
        `,
        params: [DICTATION_HISTORY_PRUNE_CAP],
      },
    ]);
  });
}

export async function setDictationHistoryPinned(
  id: string,
  pinned: boolean,
): Promise<void> {
  await enqueueDatabaseWrite(WRITE_QUEUE_KEY, async () => {
    await executeTransaction([
      {
        sql: "UPDATE dictation_history SET pinned = ? WHERE id = ?",
        params: [pinned ? 1 : 0, id],
      },
    ]);
  });
}

export async function deleteDictationHistoryEntry(id: string): Promise<void> {
  await enqueueDatabaseWrite(WRITE_QUEUE_KEY, async () => {
    await executeTransaction([
      {
        sql: "DELETE FROM dictation_history WHERE id = ?",
        params: [id],
      },
    ]);
  });
}

export async function clearDictationHistory(): Promise<void> {
  await enqueueDatabaseWrite(WRITE_QUEUE_KEY, async () => {
    await executeTransaction([
      { sql: "DELETE FROM dictation_history", params: [] },
    ]);
  });
}

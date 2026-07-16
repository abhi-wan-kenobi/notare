import type { DictationOutputMode } from "@hypr/plugin-dictation";

import { executeTransaction, useLiveQuery } from "~/db";
import { enqueueDatabaseWrite } from "~/db/write-queue";

/**
 * Dictation history - the "in-app clipboard". Every completed dictation is
 * persisted (final text, mode, cleaned-or-raw flag, timestamp) so it can be
 * re-copied later from Settings -> Dictation.
 *
 * Persistence choice: the app's SQLite DB (`dictation_history` table,
 * migration `20260716120000_dictation_history`), mirroring how every other
 * list in the app persists (chat groups/messages pattern:
 * `useLiveQuery` reads + `enqueueDatabaseWrite`-serialized transactions).
 * A store2/JSON-file store was considered and rejected: the DB gives us the
 * reactive settings UI for free and already has migration plumbing.
 */

/** Most recent entries kept; older rows are pruned on every insert. */
export const DICTATION_HISTORY_CAP = 50;

const WRITE_QUEUE_KEY = "dictation-history";

export interface DictationHistoryEntry {
  id: string;
  text: string;
  /** Output mode of the session that produced the text. */
  mode: DictationOutputMode;
  /** Whether cleanup (basic or LLM) was applied to `text`. */
  cleaned: boolean;
  /** ISO-8601 UTC timestamp. */
  createdAt: string;
}

type DictationHistorySqlRow = {
  id: string;
  text: string;
  mode: string;
  cleaned: number;
  created_at: string;
};

const EMPTY_HISTORY: DictationHistoryEntry[] = [];

export function useDictationHistory(): DictationHistoryEntry[] {
  const { data = EMPTY_HISTORY } = useLiveQuery<
    DictationHistorySqlRow,
    DictationHistoryEntry[]
  >({
    sql: `
      SELECT id, text, mode, cleaned, created_at
      FROM dictation_history
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
    mode: row.mode === "batch" ? "batch" : "type",
    cleaned: row.cleaned !== 0,
    createdAt: row.created_at,
  };
}

/**
 * Append a completed dictation and prune everything older than the newest
 * `DICTATION_HISTORY_CAP` rows in the same transaction.
 */
export async function addDictationHistoryEntry(entry: {
  text: string;
  mode: DictationOutputMode;
  cleaned: boolean;
}): Promise<void> {
  const id = crypto.randomUUID();
  const createdAt = new Date().toISOString();

  await enqueueDatabaseWrite(WRITE_QUEUE_KEY, async () => {
    await executeTransaction([
      {
        sql: `
          INSERT INTO dictation_history (id, text, mode, cleaned, created_at)
          VALUES (?, ?, ?, ?, ?)
        `,
        params: [id, entry.text, entry.mode, entry.cleaned ? 1 : 0, createdAt],
      },
      {
        sql: `
          DELETE FROM dictation_history
          WHERE id NOT IN (
            SELECT id FROM dictation_history
            ORDER BY created_at DESC, id DESC
            LIMIT ?
          )
        `,
        params: [DICTATION_HISTORY_CAP],
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

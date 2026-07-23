/**
 * Data access for the per-session action-items checklist (WS-C PR18).
 *
 * Reads use the same live-query client the rest of the app relies on and mirror
 * the canonical `list_session_action_items` filter in db-app
 * (`WHERE session_id = ? AND deleted_at IS NULL ORDER BY source_order,
 * created_at, id`), extended with the v2 columns (confidence / source_text /
 * source_start_ms / owner_speaker_id / priority) that the extraction pipeline
 * populates.
 *
 * Writes go through the same primitive `editor-bridge/task-storage` uses —
 * `enqueueDatabaseWrite("tasks", …)` + `executeTransaction` — so this panel and
 * the in-editor task list serialize against one another instead of racing.
 */

import { executeTransaction, liveQueryClient, useLiveQuery } from "~/db";
import { enqueueDatabaseWrite } from "~/db/write-queue";
import type { GatedActionItem } from "~/services/action-items/extract";

export type ActionItemStatus = "todo" | "in_progress" | "done";

export type SessionActionItemRecord = {
  id: string;
  status: ActionItemStatus;
  text: string;
  dueAt: string;
  confidence: number;
  sourceText: string;
  sourceStartMs: number | null;
  ownerSpeakerId: string;
  priority: string;
  sourceOrder: number;
};

type SessionActionItemSqlRow = {
  id: string;
  status: string;
  text: string;
  due_at: string;
  confidence: number | null;
  source_text: string;
  source_start_ms: number | null;
  owner_speaker_id: string;
  priority: string;
  source_order: number;
};

const EMPTY_ACTION_ITEMS: SessionActionItemRecord[] = [];

const SESSION_ACTION_ITEMS_SQL = `
  SELECT
    id,
    status,
    text,
    due_at,
    confidence,
    source_text,
    source_start_ms,
    owner_speaker_id,
    priority,
    source_order
  FROM action_items
  WHERE session_id = ? AND deleted_at IS NULL
  ORDER BY source_order, created_at, id
`;

function normalizeStatus(status: string): ActionItemStatus {
  return status === "done" || status === "in_progress" ? status : "todo";
}

function mapRow(row: SessionActionItemSqlRow): SessionActionItemRecord {
  const order = Number(row.source_order);
  return {
    id: row.id,
    status: normalizeStatus(row.status),
    text: row.text ?? "",
    dueAt: row.due_at ?? "",
    confidence: typeof row.confidence === "number" ? row.confidence : 0,
    sourceText: row.source_text ?? "",
    sourceStartMs:
      row.source_start_ms == null ? null : Number(row.source_start_ms),
    ownerSpeakerId: row.owner_speaker_id ?? "",
    priority: row.priority ?? "",
    sourceOrder: Number.isFinite(order) ? order : 0,
  };
}

/** Live list of a session's action items (v2 columns included). */
export function useSessionActionItems(sessionId: string): {
  items: SessionActionItemRecord[];
  isLoading: boolean;
} {
  const { data = EMPTY_ACTION_ITEMS, isLoading } = useLiveQuery<
    SessionActionItemSqlRow,
    SessionActionItemRecord[]
  >({
    sql: SESSION_ACTION_ITEMS_SQL,
    params: [sessionId],
    mapRows: (rows) => rows.map(mapRow),
    enabled: Boolean(sessionId),
  });

  return { items: data, isLoading };
}

/** Flip a single item between todo and done (in-place, no set rewrite). */
export function setActionItemStatus(
  id: string,
  status: ActionItemStatus,
  ownerUserId: string,
): Promise<void> {
  const now = new Date().toISOString();
  return enqueueDatabaseWrite("tasks", async () => {
    await executeTransaction([
      {
        sql: `
          UPDATE action_items
          SET status = ?, completed_at = ?, updated_at = ?, updated_by = ?
          WHERE id = ? AND deleted_at IS NULL
        `,
        params: [status, status === "done" ? now : null, now, ownerUserId, id],
      },
    ]);
  });
}

/**
 * Persist freshly-extracted, gated items as new session-sourced action_items
 * rows. `startOrder` is the current item count so appended rows sort after the
 * existing ones. Bodies are stored as the same paragraph JSON shape the editor
 * task bridge reads (`parseTaskBody`), so the in-note task list renders them too.
 */
export function insertSessionActionItems(
  sessionId: string,
  ownerUserId: string,
  items: GatedActionItem[],
  startOrder: number,
): Promise<void> {
  if (items.length === 0) {
    return Promise.resolve();
  }

  const now = new Date().toISOString();
  const statements = items.map((item, index) => {
    const bodyJson = JSON.stringify([
      { type: "paragraph", content: [{ type: "text", text: item.text }] },
    ]);
    return {
      sql: `
        INSERT INTO action_items (
          id, workspace_id, session_id, source_type, source_id, source_order,
          assignee_human_id, status, text, body_json, due_at,
          confidence, source_text, source_start_ms, owner_speaker_id, priority,
          synced_targets_json, created_by, updated_by, metadata_json,
          created_at, updated_at, deleted_at
        )
        VALUES (
          ?, '', ?, 'session', ?, ?,
          '', 'todo', ?, ?, ?,
          ?, ?, ?, ?, ?,
          '[]', ?, ?, '{}',
          ?, ?, NULL
        )
      `,
      params: [
        crypto.randomUUID(),
        sessionId,
        sessionId,
        startOrder + index,
        item.text,
        bodyJson,
        item.due_at,
        item.confidence,
        item.source_text,
        item.source_start_ms,
        item.owner_speaker_id,
        item.priority,
        ownerUserId,
        ownerUserId,
        now,
        now,
      ],
    };
  });

  return enqueueDatabaseWrite("tasks", async () => {
    await executeTransaction(statements);
  });
}

/** Direct (non-hook) read, used before extraction to compute the append order. */
export async function loadSessionActionItemCount(
  sessionId: string,
): Promise<number> {
  const rows = await liveQueryClient.execute<{ count: number }>(
    `SELECT COUNT(*) AS count FROM action_items WHERE session_id = ? AND deleted_at IS NULL`,
    [sessionId],
  );
  return Number(rows[0]?.count ?? 0);
}

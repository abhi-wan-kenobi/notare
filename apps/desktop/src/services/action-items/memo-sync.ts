/**
 * Inbound half of the fs-sync action-items round-trip (WS-D2).
 *
 * The memo's `## Action Items` section is a rendered projection of SQLite
 * (outbound, see fs-materializer). This module reads an *external* edit to that
 * section back into SQLite — but ONLY the checkbox state (`- [ ]`↔`- [x]`),
 * mapped to `action_items.status` / `completed_at`.
 *
 * KNOWN LIMITATION — text edits are NOT synced back. Items are matched to rows
 * by their text identity (the same `text` the outbound renderer emits), so if a
 * user rewrites a line's text on disk it simply won't match a row and the toggle
 * is ignored. Only the checkbox flip is authoritative inbound.
 */

import { parseActionItemsSection } from "./section";

import { executeTransaction, liveQueryClient } from "~/db";

/** The minimal SQLite `action_items` shape needed to reconcile a toggle. */
export type ActionItemToggleRow = {
  id: string;
  text: string;
  status: string;
};

/** A single status change to apply back to SQLite. */
export type ActionItemToggleUpdate = {
  id: string;
  status: string;
  completedAt: string | null;
};

export type MemoSyncDependencies = {
  execute: typeof liveQueryClient.execute;
  executeTransaction: typeof executeTransaction;
  now: () => string;
};

const defaultDependencies: MemoSyncDependencies = {
  execute: liveQueryClient.execute.bind(liveQueryClient),
  executeTransaction,
  now: () => new Date().toISOString(),
};

function isDone(status: string): boolean {
  return status === "done" || status === "completed";
}

/**
 * Pure diff: given the current SQLite rows and the checkbox states parsed from
 * the memo, return the status updates needed to make SQLite agree with the
 * checkboxes. Matching is by trimmed text identity; the first row wins on
 * duplicate text, and each row is updated at most once.
 */
export function reconcileActionItemToggles(
  rows: ActionItemToggleRow[],
  states: Array<{ text: string; done: boolean }>,
  now: string,
): ActionItemToggleUpdate[] {
  const byText = new Map<string, ActionItemToggleRow>();
  for (const row of rows) {
    const key = row.text.trim();
    if (!byText.has(key)) {
      byText.set(key, row);
    }
  }

  const seen = new Set<string>();
  const updates: ActionItemToggleUpdate[] = [];
  for (const state of states) {
    const row = byText.get(state.text.trim());
    if (!row || seen.has(row.id)) {
      continue;
    }
    if (isDone(row.status) === state.done) {
      continue;
    }
    seen.add(row.id);
    updates.push(
      state.done
        ? { id: row.id, status: "done", completedAt: now }
        : { id: row.id, status: "todo", completedAt: null },
    );
  }
  return updates;
}

/**
 * Read the memo's Action Items region and apply any checkbox toggles back into
 * SQLite for `sessionId`. Returns the updates that were applied (empty when
 * nothing changed). Text edits are ignored (see module docs).
 */
export async function syncMemoActionItemToggles(
  sessionId: string,
  memoMarkdown: string,
  dependencies: MemoSyncDependencies = defaultDependencies,
): Promise<ActionItemToggleUpdate[]> {
  const states = parseActionItemsSection(memoMarkdown);
  if (states.length === 0) {
    return [];
  }

  const rows = await dependencies.execute<ActionItemToggleRow>(
    `
      SELECT id, text, status
      FROM action_items
      WHERE session_id = ? AND deleted_at IS NULL
    `,
    [sessionId],
  );

  const now = dependencies.now();
  const updates = reconcileActionItemToggles(rows, states, now);
  if (updates.length === 0) {
    return [];
  }

  await dependencies.executeTransaction(
    updates.map((update) => ({
      sql: `
        UPDATE action_items
        SET status = ?, completed_at = ?, updated_at = ?
        WHERE id = ? AND deleted_at IS NULL
      `,
      params: [update.status, update.completedAt, now, update.id],
    })),
  );

  return updates;
}

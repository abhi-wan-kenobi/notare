import { useLiveQuery } from "~/db";
import { useOwnerUserId } from "~/shared/owner-user";

export type AllActionItemFilter = "open" | "mine" | "overdue" | "done";

export type AllActionItem = {
  id: string;
  text: string;
  status: string;
  due_at: string;
  assignee_human_id: string;
  owner_name: string;
  session_id: string;
  session_title: string;
  session_date: string;
  series_id: string;
  carried_from_session_id?: string | null;
  carried_from_session_title?: string;
  carried_from_session_date?: string;
};

const EMPTY: AllActionItem[] = [];

export function useAllActionItems(filter: AllActionItemFilter, userId?: string) {
  const currentOwnerUserId = useOwnerUserId() ?? "";
  const effectiveUserId = userId ?? currentOwnerUserId;

  const { data: hasCarriedCol = false } = useLiveQuery<
    { has_col: number },
    boolean
  >({
    sql: `
      SELECT COUNT(*) AS has_col
      FROM sqlite_master
      WHERE type = 'table' AND name = 'action_items' AND sql LIKE '%carried_from_session_id%'
    `,
    mapRows: (rows) => Number(rows[0]?.has_col ?? 0) > 0,
  });

  const sql = hasCarriedCol
    ? `
      SELECT ai.id, ai.text, ai.status, ai.due_at, ai.assignee_human_id,
        COALESCE(h.name, '') AS owner_name, s.id AS session_id,
        s.title AS session_title, s.started_at AS session_date, s.series_id,
        ai.carried_from_session_id,
        COALESCE(cs.title, '') AS carried_from_session_title,
        COALESCE(cs.started_at, '') AS carried_from_session_date
      FROM action_items ai
      JOIN sessions s ON s.id = ai.session_id AND s.deleted_at IS NULL
      LEFT JOIN humans h ON h.id = ai.assignee_human_id AND h.deleted_at IS NULL
      LEFT JOIN sessions cs ON cs.id = ai.carried_from_session_id AND cs.deleted_at IS NULL
      WHERE ai.deleted_at IS NULL
        AND ((? = 'done' AND ai.status = 'done') OR (? != 'done' AND ai.status != 'done'))
        AND (? != 'mine' OR ai.assignee_human_id = ? OR ai.assignee_human_id IN (SELECT id FROM humans WHERE owner_user_id = ?))
        AND (? != 'overdue' OR (ai.due_at != '' AND ai.due_at < ?))
      ORDER BY s.started_at DESC, s.created_at DESC, ai.source_order, ai.created_at
    `
    : `
      SELECT ai.id, ai.text, ai.status, ai.due_at, ai.assignee_human_id,
        COALESCE(h.name, '') AS owner_name, s.id AS session_id,
        s.title AS session_title, s.started_at AS session_date, s.series_id,
        NULL AS carried_from_session_id,
        '' AS carried_from_session_title,
        '' AS carried_from_session_date
      FROM action_items ai
      JOIN sessions s ON s.id = ai.session_id AND s.deleted_at IS NULL
      LEFT JOIN humans h ON h.id = ai.assignee_human_id AND h.deleted_at IS NULL
      WHERE ai.deleted_at IS NULL
        AND ((? = 'done' AND ai.status = 'done') OR (? != 'done' AND ai.status != 'done'))
        AND (? != 'mine' OR ai.assignee_human_id = ? OR ai.assignee_human_id IN (SELECT id FROM humans WHERE owner_user_id = ?))
        AND (? != 'overdue' OR (ai.due_at != '' AND ai.due_at < ?))
      ORDER BY s.started_at DESC, s.created_at DESC, ai.source_order, ai.created_at
    `;

  const todayStr = new Date().toISOString().slice(0, 10);
  const params = [
    filter,
    filter,
    filter,
    effectiveUserId,
    effectiveUserId,
    filter,
    todayStr,
  ];

  const { data = EMPTY, isLoading } = useLiveQuery<
    AllActionItem,
    AllActionItem[]
  >({
    sql,
    params,
  });

  return { items: data, isLoading };
}

export function useAllOpenActionItemsCount(): number {
  const { data = 0 } = useLiveQuery<{ count: number }, number>({
    sql: `
      SELECT COUNT(*) AS count
      FROM action_items
      WHERE deleted_at IS NULL AND status != 'done'
    `,
    mapRows: (rows) => Number(rows[0]?.count ?? 0),
  });
  return data;
}

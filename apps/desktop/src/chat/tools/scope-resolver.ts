import type { ToolDependencies } from "./types";

import type { ToolScopeFilter } from "~/chat/state/chat-scope";
import { liveQueryClient } from "~/db";

export async function loadSessionIdsForHuman(
  humanId: string,
): Promise<string[]> {
  const rows = await liveQueryClient.execute<{ id: string }>(
    `SELECT DISTINCT sessions.id
     FROM session_participants
     INNER JOIN sessions ON sessions.id = session_participants.session_id
     WHERE session_participants.human_id = ?
       AND session_participants.source <> 'excluded'
       AND session_participants.deleted_at IS NULL
       AND sessions.deleted_at IS NULL
     ORDER BY sessions.created_at DESC`,
    [humanId],
  );
  return rows.map((r) => r.id);
}

export async function loadSessionIdsForOrg(orgId: string): Promise<string[]> {
  const rows = await liveQueryClient.execute<{ id: string }>(
    `SELECT DISTINCT sessions.id
     FROM session_participants
     INNER JOIN sessions ON sessions.id = session_participants.session_id
     INNER JOIN humans ON humans.id = session_participants.human_id
     WHERE humans.organization_id = ?
       AND session_participants.source <> 'excluded'
       AND session_participants.deleted_at IS NULL
       AND humans.deleted_at IS NULL
       AND sessions.deleted_at IS NULL
     ORDER BY sessions.created_at DESC`,
    [orgId],
  );
  return rows.map((r) => r.id);
}

export async function resolveAllowedSessionIds(
  filter: ToolScopeFilter | undefined,
  deps?: ToolDependencies,
): Promise<string[] | null> {
  if (!filter || filter.kind === "none") {
    return null;
  }

  if (filter.kind === "session_ids") {
    return filter.sessionIds;
  }

  if (filter.kind === "human_id") {
    if (deps?.resolveSessionIdsForHuman) {
      return await deps.resolveSessionIdsForHuman(filter.humanId);
    }
    return await loadSessionIdsForHuman(filter.humanId);
  }

  if (filter.kind === "org_id") {
    if (deps?.resolveSessionIdsForOrg) {
      return await deps.resolveSessionIdsForOrg(filter.orgId);
    }
    return await loadSessionIdsForOrg(filter.orgId);
  }

  return null;
}

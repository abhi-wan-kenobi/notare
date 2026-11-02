import type { SessionBrief } from "./types";

import { executeTransaction, liveQueryClient, useLiveQuery } from "~/db";
import { enqueueDatabaseWrite } from "~/db/write-queue";
import { id } from "~/shared/utils";

type SessionBriefSqlRow = {
  id: string;
  session_id: string;
  event_id: string;
  generated_at: string;
  content_md: string;
  model: string;
  dismissed_at: string | null;
  created_at: string;
  updated_at: string;
};

function mapRow(row: SessionBriefSqlRow | undefined): SessionBrief | null {
  if (!row) return null;
  return {
    id: row.id,
    session_id: row.session_id,
    event_id: row.event_id,
    generated_at: row.generated_at,
    content_md: row.content_md,
    model: row.model,
    dismissed_at: row.dismissed_at,
    created_at: row.created_at,
    updated_at: row.updated_at,
  };
}

export async function loadBriefForEvent(
  eventId: string,
): Promise<SessionBrief | null> {
  if (!eventId) return null;
  const rows = await liveQueryClient.execute<SessionBriefSqlRow>(
    `
      SELECT
        id, session_id, event_id, generated_at, content_md, model, dismissed_at, created_at, updated_at
      FROM session_briefs
      WHERE event_id = ? AND dismissed_at IS NULL
      ORDER BY generated_at DESC
      LIMIT 1
    `,
    [eventId],
  );
  return mapRow(rows[0]);
}

export async function loadBriefForSession(
  sessionId: string,
): Promise<SessionBrief | null> {
  if (!sessionId) return null;
  const rows = await liveQueryClient.execute<SessionBriefSqlRow>(
    `
      SELECT
        id, session_id, event_id, generated_at, content_md, model, dismissed_at, created_at, updated_at
      FROM session_briefs
      WHERE (
        session_id = ?
        OR (event_id <> '' AND event_id = (
          SELECT COALESCE(NULLIF(event_id, ''), NULLIF(external_event_id, ''), '')
          FROM sessions
          WHERE id = ? AND deleted_at IS NULL
          LIMIT 1
        ))
      )
        AND dismissed_at IS NULL
      ORDER BY generated_at DESC
      LIMIT 1
    `,
    [sessionId, sessionId],
  );
  return mapRow(rows[0]);
}

export async function loadBriefedEventIds(): Promise<Set<string>> {
  const rows = await liveQueryClient.execute<{ event_id: string }>(`
    SELECT DISTINCT event_id
    FROM session_briefs
    WHERE event_id <> '' AND dismissed_at IS NULL
  `);
  return new Set(rows.map((r) => r.event_id));
}

export async function saveBrief(params: {
  id?: string;
  session_id?: string;
  event_id: string;
  content_md: string;
  model: string;
}): Promise<SessionBrief> {
  return enqueueDatabaseWrite("save-brief", async () => {
    const existing = await loadBriefForEvent(params.event_id);
    const briefId = params.id ?? existing?.id ?? id();
    const now = new Date().toISOString();
    const sessionId = params.session_id ?? existing?.session_id ?? "";

    await executeTransaction([
      {
        sql: `
          INSERT INTO session_briefs (
            id, session_id, event_id, generated_at, content_md, model, dismissed_at, created_at, updated_at
          ) VALUES (?, ?, ?, ?, ?, ?, NULL, ?, ?)
          ON CONFLICT(id) DO UPDATE SET
            session_id = CASE WHEN excluded.session_id <> '' THEN excluded.session_id ELSE session_briefs.session_id END,
            event_id = excluded.event_id,
            generated_at = excluded.generated_at,
            content_md = excluded.content_md,
            model = excluded.model,
            dismissed_at = NULL,
            updated_at = excluded.updated_at
        `,
        params: [
          briefId,
          sessionId,
          params.event_id,
          now,
          params.content_md,
          params.model,
          now,
          now,
        ],
      },
    ]);

    return {
      id: briefId,
      session_id: sessionId,
      event_id: params.event_id,
      generated_at: now,
      content_md: params.content_md,
      model: params.model,
      dismissed_at: null,
      created_at: existing?.created_at ?? now,
      updated_at: now,
    };
  });
}

export async function dismissBrief(briefId: string): Promise<void> {
  if (!briefId) return;
  await enqueueDatabaseWrite("dismiss-brief", async () => {
    const now = new Date().toISOString();
    await executeTransaction([
      {
        sql: `UPDATE session_briefs SET dismissed_at = ?, updated_at = ? WHERE id = ?`,
        params: [now, now, briefId],
      },
    ]);
  });
}

export function useBriefForEvent(
  eventId: string | undefined,
): SessionBrief | null {
  const { data = null } = useLiveQuery<SessionBriefSqlRow, SessionBrief | null>(
    {
      enabled: Boolean(eventId),
      sql: `
      SELECT
        id, session_id, event_id, generated_at, content_md, model, dismissed_at, created_at, updated_at
      FROM session_briefs
      WHERE event_id = ? AND dismissed_at IS NULL
      ORDER BY generated_at DESC
      LIMIT 1
    `,
      params: [eventId ?? ""],
      mapRows: (rows) => mapRow(rows[0]),
    },
  );

  return eventId ? data : null;
}

export function useBriefForSession(
  sessionId: string | undefined,
): SessionBrief | null {
  const { data = null } = useLiveQuery<SessionBriefSqlRow, SessionBrief | null>(
    {
      enabled: Boolean(sessionId),
      sql: `
      SELECT
        id, session_id, event_id, generated_at, content_md, model, dismissed_at, created_at, updated_at
      FROM session_briefs
      WHERE (
        session_id = ?
        OR (event_id <> '' AND event_id = (
          SELECT COALESCE(NULLIF(event_id, ''), NULLIF(external_event_id, ''), '')
          FROM sessions
          WHERE id = ? AND deleted_at IS NULL
          LIMIT 1
        ))
      )
        AND dismissed_at IS NULL
      ORDER BY generated_at DESC
      LIMIT 1
    `,
      params: [sessionId ?? "", sessionId ?? ""],
      mapRows: (rows) => mapRow(rows[0]),
    },
  );

  return sessionId ? data : null;
}

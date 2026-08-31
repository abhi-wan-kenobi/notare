import { useCallback } from "react";

import { json2md, md2json } from "@hypr/editor/markdown";
import { commands as analyticsCommands } from "@hypr/plugin-analytics";
import { commands as fsSyncCommands } from "@hypr/plugin-fs-sync";
import type { EventParticipant, SessionEvent } from "@hypr/store";

import { executeTransaction, liveQueryClient, useLiveQuery } from "~/db";
import { enqueueDatabaseWrite } from "~/db/write-queue";
import { DEFAULT_USER_ID, id } from "~/shared/utils";
import type { DeletedSessionData } from "~/store/zustand/undo-delete";

type EventSqlRow = {
  id: string;
  tracking_id_event: string;
  calendar_id: string;
  title: string;
  started_at: string;
  ended_at: string;
  location: string;
  meeting_link: string;
  description: string;
  recurrence_series_id: string;
  has_recurrence_rules: boolean | number;
  is_all_day: boolean | number;
  provider: string;
  participants_json: string | null;
};

type HumanEmailSqlRow = { id: string; email: string };
type SessionIdentitySqlRow = { id: string };
type SessionEventSqlRow = { event_json: string };
type SessionDeleteSqlRow = { id: string; title: string };
type SessionEmptySqlRow = {
  title: string;
  event_json: string;
  note_body: string;
  note_body_format: string;
  transcript_count: number;
  enhanced_note_count: number;
  manual_participant_count: number;
  tag_count: number;
};

type SessionSqlRow = {
  id: string;
  owner_user_id: string;
  created_at: string;
  folder_path: string;
  event_json: string;
  title: string;
  raw_body: string;
  raw_body_format: string;
};

type SessionSummarySqlRow = {
  id: string;
  title: string;
  created_at: string;
};

type TrashedSessionSqlRow = {
  id: string;
  title: string;
  created_at: string;
  deleted_at: string;
  note_body: string;
  note_body_format: string;
};

type SessionTranscriptStateSqlRow = {
  has_transcript: boolean | number;
};

type SessionParticipantSqlRow = {
  id: string;
  session_id: string;
  human_id: string;
  source: string;
  name: string;
  email: string;
  job_title: string;
  linkedin_username: string;
  organization_id: string;
  organization_name: string;
};

type EnhancedNoteSqlRow = {
  id: string;
  session_id: string;
  title: string;
  body: string;
  body_format: string;
  template_id: string;
  sort_order: number;
};

export type SessionRecord = {
  id: string;
  user_id: string;
  created_at: string;
  folder_id: string;
  event_json: string;
  title: string;
  raw_md: string;
};

export type SessionChanges = Partial<
  Pick<
    SessionRecord,
    "created_at" | "event_json" | "folder_id" | "raw_md" | "title"
  >
>;

export type SessionSummaryRecord = {
  id: string;
  title: string;
  created_at: string;
};

export type TrashedSessionRecord = {
  id: string;
  title: string;
  created_at: string;
  deleted_at: string;
  preview: string;
};

export type EnhancedNoteRecord = {
  id: string;
  sessionId: string;
  title: string;
  content: string;
  templateId: string;
  position: number;
};

export type SessionParticipantRecord = {
  id: string;
  sessionId: string;
  humanId: string;
  source: string;
  name: string;
  email: string;
  jobTitle: string;
  linkedinUsername: string;
  organizationId: string;
  organizationName: string;
};

const EMPTY_ENHANCED_NOTES: EnhancedNoteRecord[] = [];
const EMPTY_SESSION_PARTICIPANTS: SessionParticipantRecord[] = [];
const EMPTY_SESSION_SUMMARIES: SessionSummaryRecord[] = [];
const EMPTY_TRASHED_SESSIONS: TrashedSessionRecord[] = [];

/**
 * Every table that carries session-owned rows and gets a `deleted_at`
 * tombstone from `buildSessionTombstoneStatements`. Kept as one list so the
 * soft-delete, restore and hard-delete paths can never drift apart.
 */
const SESSION_OWNED_TABLES = [
  "session_documents",
  "transcripts",
  "session_participants",
  "session_tags",
  "action_items",
  "session_attachments",
] as const;

/**
 * Single-line plain-text rendering of a stored note body for list previews.
 * Mirrors `bodyToMarkdown` in `session/content-queries.ts` (prosemirror_json
 * bodies go through `json2md`), then strips markdown noise so a trashed
 * session row can show what its note actually said.
 */
function noteBodyToPlainText(body: string, format: string): string {
  if (!body || format === "markdown") return body;
  try {
    return json2md(JSON.parse(body));
  } catch {
    return body;
  }
}

const SESSION_SELECT_SQL = `
  SELECT
    sessions.id,
    sessions.owner_user_id,
    sessions.created_at,
    sessions.folder_path,
    sessions.event_json,
    sessions.title,
    COALESCE(note.body, '') AS raw_body,
    COALESCE(note.body_format, 'prosemirror_json') AS raw_body_format
  FROM sessions
  LEFT JOIN session_documents AS note
    ON note.id = sessions.id
    AND note.kind = 'note'
    AND note.deleted_at IS NULL
  WHERE sessions.id = ? AND sessions.deleted_at IS NULL
  LIMIT 1
`;

export function useSession(sessionId: string): SessionRecord | null {
  const { data = null } = useLiveQuery<SessionSqlRow, SessionRecord | null>({
    sql: SESSION_SELECT_SQL,
    params: [sessionId],
    enabled: Boolean(sessionId),
    mapRows: (rows) => {
      const row = rows[0];
      return row ? mapSessionRow(row) : null;
    },
  });
  return sessionId ? data : null;
}

export function useSessionSummary(
  sessionId: string,
): SessionSummaryRecord | null {
  const { data = null } = useLiveQuery<
    SessionSummarySqlRow,
    SessionSummaryRecord | null
  >({
    sql: `
      SELECT id, title, created_at
      FROM sessions
      WHERE id = ? AND deleted_at IS NULL
      LIMIT 1
    `,
    params: [sessionId],
    enabled: Boolean(sessionId),
    mapRows: (rows) => rows[0] ?? null,
  });
  return sessionId ? data : null;
}

export function useSessionSummaries(): SessionSummaryRecord[] {
  const { data = EMPTY_SESSION_SUMMARIES } = useLiveQuery<
    SessionSummarySqlRow,
    SessionSummaryRecord[]
  >({
    sql: `
      SELECT id, title, created_at
      FROM sessions
      WHERE deleted_at IS NULL
      ORDER BY created_at DESC, id
    `,
  });
  return data;
}

export function useTrashedSessions(): TrashedSessionRecord[] {
  const { data = EMPTY_TRASHED_SESSIONS } = useLiveQuery<
    TrashedSessionSqlRow,
    TrashedSessionRecord[]
  >({
    sql: `
      SELECT
        sessions.id,
        sessions.title,
        sessions.created_at,
        sessions.deleted_at,
        COALESCE(note.body, '') AS note_body,
        COALESCE(note.body_format, 'prosemirror_json') AS note_body_format
      FROM sessions
      LEFT JOIN session_documents AS note
        ON note.id = sessions.id
        AND note.kind = 'note'
      WHERE sessions.deleted_at IS NOT NULL
      ORDER BY sessions.deleted_at DESC, sessions.id
    `,
    mapRows: (rows) =>
      rows.map((row) => ({
        id: row.id,
        title: row.title,
        created_at: row.created_at,
        deleted_at: row.deleted_at,
        preview: noteBodyToPlainText(row.note_body, row.note_body_format),
      })),
  });
  return data;
}

export async function loadSessionEvent(
  sessionId: string,
): Promise<SessionEvent | null> {
  const rows = await liveQueryClient.execute<SessionEventSqlRow>(
    `
      SELECT event_json
      FROM sessions
      WHERE id = ? AND deleted_at IS NULL
      LIMIT 1
    `,
    [sessionId],
  );
  const eventJson = rows[0]?.event_json;
  if (!eventJson) return null;

  try {
    return JSON.parse(eventJson) as SessionEvent;
  } catch {
    return null;
  }
}

export function useUpdateSession(sessionId: string) {
  return useCallback(
    (changes: SessionChanges) => updateSession(sessionId, changes),
    [sessionId],
  );
}

export function useSessionHasTranscript(sessionId: string): boolean {
  const { data = false } = useLiveQuery<SessionTranscriptStateSqlRow, boolean>({
    sql: `
      SELECT EXISTS (
        SELECT 1
        FROM transcripts
        WHERE session_id = ?
          AND deleted_at IS NULL
          AND CASE
            WHEN json_valid(words_json) THEN json_array_length(words_json)
            ELSE 0
          END > 0
      ) AS has_transcript
    `,
    params: [sessionId],
    enabled: Boolean(sessionId),
    mapRows: (rows) => Boolean(rows[0]?.has_transcript),
  });
  return sessionId ? data : false;
}

export function useSessionParticipants(
  sessionId: string,
): SessionParticipantRecord[] {
  const { data = EMPTY_SESSION_PARTICIPANTS } = useLiveQuery<
    SessionParticipantSqlRow,
    SessionParticipantRecord[]
  >({
    sql: `
      SELECT
        participant.id,
        participant.session_id,
        participant.human_id,
        participant.source,
        COALESCE(NULLIF(human.name, ''), participant.display_name) AS name,
        COALESCE(NULLIF(human.email, ''), participant.email) AS email,
        COALESCE(human.job_title, '') AS job_title,
        COALESCE(human.linkedin_username, '') AS linkedin_username,
        COALESCE(human.organization_id, '') AS organization_id,
        COALESCE(organization.name, '') AS organization_name
      FROM session_participants AS participant
      LEFT JOIN humans AS human
        ON human.id = participant.human_id AND human.deleted_at IS NULL
      LEFT JOIN organizations AS organization
        ON organization.id = human.organization_id
        AND organization.deleted_at IS NULL
      WHERE participant.session_id = ?
        AND participant.deleted_at IS NULL
      ORDER BY name, email, participant.id
    `,
    params: [sessionId],
    enabled: Boolean(sessionId),
    mapRows: (rows) => rows.map(mapSessionParticipantRow),
  });
  return sessionId ? data : EMPTY_SESSION_PARTICIPANTS;
}

export function useSessionParticipant(
  mappingId: string,
): SessionParticipantRecord | null {
  const { data = null } = useLiveQuery<
    SessionParticipantSqlRow,
    SessionParticipantRecord | null
  >({
    sql: `
      SELECT
        participant.id,
        participant.session_id,
        participant.human_id,
        participant.source,
        COALESCE(NULLIF(human.name, ''), participant.display_name) AS name,
        COALESCE(NULLIF(human.email, ''), participant.email) AS email,
        COALESCE(human.job_title, '') AS job_title,
        COALESCE(human.linkedin_username, '') AS linkedin_username,
        COALESCE(human.organization_id, '') AS organization_id,
        COALESCE(organization.name, '') AS organization_name
      FROM session_participants AS participant
      LEFT JOIN humans AS human
        ON human.id = participant.human_id AND human.deleted_at IS NULL
      LEFT JOIN organizations AS organization
        ON organization.id = human.organization_id
        AND organization.deleted_at IS NULL
      WHERE participant.id = ? AND participant.deleted_at IS NULL
      LIMIT 1
    `,
    params: [mappingId],
    enabled: Boolean(mappingId),
    mapRows: (rows) => (rows[0] ? mapSessionParticipantRow(rows[0]) : null),
  });
  return mappingId ? data : null;
}

export function addSessionParticipant(
  sessionId: string,
  humanId: string,
  source = "manual",
): Promise<void> {
  return enqueueDatabaseWrite("session-participants", async () => {
    const participantId = id();
    const now = new Date().toISOString();
    await executeTransaction([
      {
        sql: `
          UPDATE session_participants
          SET source = ?, updated_at = ?
          WHERE id = (
            SELECT id
            FROM session_participants
            WHERE session_id = ?
              AND human_id = ?
              AND source = 'excluded'
              AND deleted_at IS NULL
              AND ? <> 'auto'
            ORDER BY created_at, id
            LIMIT 1
          )
        `,
        params: [source, now, sessionId, humanId, source],
      },
      {
        sql: `
          INSERT INTO session_participants (
            id, workspace_id, owner_user_id, session_id, human_id,
            display_name, email, role, source, metadata_json, created_at,
            updated_at, deleted_at
          )
          SELECT ?, '', session.owner_user_id, session.id, human.id,
            human.name, human.email, '', ?, '{}', ?, ?, NULL
          FROM sessions AS session
          JOIN humans AS human ON human.id = ? AND human.deleted_at IS NULL
          WHERE session.id = ?
            AND session.deleted_at IS NULL
            AND NOT EXISTS (
              SELECT 1
              FROM session_participants AS existing
              WHERE existing.session_id = session.id
                AND existing.human_id = human.id
                AND existing.deleted_at IS NULL
            )
        `,
        params: [participantId, source, now, now, humanId, sessionId],
      },
    ]);
  });
}

export function removeSessionParticipant(mappingId: string): Promise<void> {
  return enqueueDatabaseWrite("session-participants", async () => {
    const now = new Date().toISOString();
    await executeTransaction([
      {
        sql: `
          UPDATE session_participants
          SET
            source = CASE WHEN source = 'auto' THEN 'excluded' ELSE source END,
            deleted_at = CASE WHEN source = 'auto' THEN NULL ELSE ? END,
            updated_at = ?
          WHERE id = ? AND deleted_at IS NULL
        `,
        params: [now, now, mappingId],
      },
    ]);
  });
}

export function useEnhancedNoteRecords(
  sessionId: string,
): EnhancedNoteRecord[] {
  const { data = EMPTY_ENHANCED_NOTES } = useLiveQuery<
    EnhancedNoteSqlRow,
    EnhancedNoteRecord[]
  >({
    sql: `
      SELECT
        id,
        session_id,
        title,
        body,
        body_format,
        template_id,
        sort_order
      FROM session_documents
      WHERE session_id = ?
        AND kind IN ('summary', 'template_output')
        AND deleted_at IS NULL
      ORDER BY sort_order, id
    `,
    params: [sessionId],
    enabled: Boolean(sessionId),
    mapRows: (rows) => rows.map(mapEnhancedNoteRow),
  });
  return sessionId ? data : EMPTY_ENHANCED_NOTES;
}

export function useEnhancedNote(
  enhancedNoteId: string,
): EnhancedNoteRecord | null {
  const { data = null } = useLiveQuery<
    EnhancedNoteSqlRow,
    EnhancedNoteRecord | null
  >({
    sql: `
      SELECT
        id,
        session_id,
        title,
        body,
        body_format,
        template_id,
        sort_order
      FROM session_documents
      WHERE id = ?
        AND kind IN ('summary', 'template_output')
        AND deleted_at IS NULL
      LIMIT 1
    `,
    params: [enhancedNoteId],
    enabled: Boolean(enhancedNoteId),
    mapRows: (rows) => {
      const row = rows[0];
      return row ? mapEnhancedNoteRow(row) : null;
    },
  });
  return enhancedNoteId ? data : null;
}

export function useUpdateEnhancedNoteContent(
  enhancedNoteId: string,
  sessionId: string,
) {
  return useCallback(
    (content: string, sessionTitle?: string) =>
      updateEnhancedNoteContent(
        enhancedNoteId,
        sessionId,
        content,
        sessionTitle,
      ),
    [enhancedNoteId, sessionId],
  );
}

export function updateEnhancedNoteContent(
  enhancedNoteId: string,
  sessionId: string,
  content: string,
  sessionTitle?: string,
): Promise<void> {
  return enqueueDatabaseWrite(`session:${sessionId}`, async () => {
    const now = new Date().toISOString();
    const statements: Array<{ sql: string; params: unknown[] }> = [
      {
        sql: `
          UPDATE session_documents
          SET body = ?, body_format = 'prosemirror_json', updated_at = ?
          WHERE id = ?
            AND kind IN ('summary', 'template_output')
            AND deleted_at IS NULL
        `,
        params: [content, now, enhancedNoteId],
      },
    ];

    if (sessionTitle !== undefined) {
      statements.push({
        sql: `
          UPDATE sessions
          SET title = ?, updated_at = ?
          WHERE id = ? AND deleted_at IS NULL
        `,
        params: [sessionTitle, now, sessionId],
      });
    }

    await executeTransaction(statements);
  });
}

export function deleteEnhancedNote(enhancedNoteId: string): Promise<void> {
  return enqueueDatabaseWrite(`enhanced-note:${enhancedNoteId}`, async () => {
    const now = new Date().toISOString();
    await executeTransaction([
      {
        sql: `
          UPDATE session_documents
          SET deleted_at = ?, updated_at = ?
          WHERE id = ?
            AND kind IN ('summary', 'template_output')
            AND deleted_at IS NULL
        `,
        params: [now, now, enhancedNoteId],
      },
    ]);
  });
}

export function updateSession(
  sessionId: string,
  changes: SessionChanges,
): Promise<void> {
  return enqueueDatabaseWrite(`session:${sessionId}`, async () => {
    const now = new Date().toISOString();
    const assignments: string[] = [];
    const params: unknown[] = [];

    for (const [column, value] of [
      ["title", changes.title],
      ["created_at", changes.created_at],
      ["folder_path", changes.folder_id],
      ["event_json", changes.event_json],
    ] as const) {
      if (value === undefined) continue;
      assignments.push(`${column} = ?`);
      params.push(value);
    }

    const statements: Array<{ sql: string; params: unknown[] }> = [];
    if (assignments.length > 0) {
      statements.push({
        sql: `
          UPDATE sessions
          SET ${assignments.join(", ")}, updated_at = ?
          WHERE id = ? AND deleted_at IS NULL
        `,
        params: [...params, now, sessionId],
      });
    }

    if (changes.raw_md !== undefined) {
      statements.push({
        sql: `
          INSERT INTO session_documents (
            id, session_id, kind, body_format, body, created_by, updated_by,
            created_at, updated_at, deleted_at
          )
          SELECT ?, ?, 'note', 'prosemirror_json', ?, owner_user_id,
            owner_user_id, ?, ?, NULL
          FROM sessions
          WHERE id = ? AND deleted_at IS NULL
          ON CONFLICT(id) DO UPDATE SET
            body_format = excluded.body_format,
            body = excluded.body,
            updated_by = excluded.updated_by,
            updated_at = excluded.updated_at,
            deleted_at = NULL
        `,
        params: [sessionId, sessionId, changes.raw_md, now, now, sessionId],
      });
    }

    if (statements.length > 0) await executeTransaction(statements);
  });
}

export async function createSession(
  title = "",
  userId = DEFAULT_USER_ID,
  initial?: Pick<SessionChanges, "event_json" | "raw_md">,
): Promise<string> {
  const sessionId = id();
  const participantId = id();
  const now = new Date().toISOString();

  await executeTransaction([
    {
      sql: `
        INSERT INTO sessions (
          id, owner_user_id, title, event_json, created_at, updated_at,
          deleted_at
        ) VALUES (?, ?, ?, ?, ?, ?, NULL)
      `,
      params: [sessionId, userId, title, initial?.event_json ?? "", now, now],
    },
    createEmptyNoteStatement(
      sessionId,
      userId,
      now,
      false,
      initial?.raw_md ?? "",
    ),
    {
      sql: `
        INSERT INTO humans (id, owner_user_id, updated_at, deleted_at)
        VALUES (?, ?, ?, NULL)
        ON CONFLICT(id) DO UPDATE SET
          deleted_at = NULL,
          updated_at = excluded.updated_at
      `,
      params: [userId, userId, now],
    },
    {
      sql: `
        INSERT INTO session_participants (
          id, owner_user_id, session_id, human_id, source, created_at,
          updated_at, deleted_at
        ) VALUES (?, ?, ?, ?, 'manual', ?, ?, NULL)
      `,
      params: [participantId, userId, sessionId, userId, now, now],
    },
  ]);

  await trackNoteCreated(false);
  return sessionId;
}

export async function getOrCreateSessionForEventId(
  eventId: string,
  title?: string,
  userId = DEFAULT_USER_ID,
): Promise<string> {
  const [event] = await liveQueryClient.execute<EventSqlRow>(
    `
      SELECT
        id,
        tracking_id_event,
        calendar_id,
        title,
        started_at,
        ended_at,
        location,
        meeting_link,
        description,
        recurrence_series_id,
        has_recurrence_rules,
        is_all_day,
        provider,
        participants_json
      FROM events
      WHERE id = ? AND deleted_at IS NULL
      LIMIT 1
    `,
    [eventId],
  );

  if (!event) {
    return createSession(title, userId);
  }

  const existingSessionId = await findSessionForEvent(event);
  if (existingSessionId) {
    return existingSessionId;
  }

  const sessionId = id();
  const now = new Date().toISOString();
  const sessionEvent = toSessionEvent(event);
  const participants = parseEventParticipants(event.participants_json);
  const humansByEmail = await findHumansByEmail(participants);
  const statements = [
    {
      sql: `
        INSERT INTO sessions (
          id, owner_user_id, title, created_at, updated_at, started_at,
          ended_at, event_id, external_event_id, external_provider, series_id,
          event_json, deleted_at
        )
        SELECT ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL
        WHERE NOT EXISTS (
          SELECT 1
          FROM sessions
          WHERE deleted_at IS NULL
            AND (event_id = ? OR (? <> '' AND external_event_id = ?))
        )
      `,
      params: [
        sessionId,
        userId,
        title ?? sessionEvent.title,
        now,
        now,
        sessionEvent.started_at,
        sessionEvent.ended_at,
        event.id,
        event.tracking_id_event,
        event.provider,
        event.recurrence_series_id,
        JSON.stringify(sessionEvent),
        event.id,
        event.tracking_id_event,
        event.tracking_id_event,
      ],
    },
    createEmptyNoteStatement(sessionId, userId, now, true),
  ];

  const seenEmails = new Set<string>();
  for (const participant of participants) {
    const email = participant.email?.trim();
    if (!email) continue;
    const emailKey = email.toLowerCase();
    if (seenEmails.has(emailKey)) continue;
    seenEmails.add(emailKey);

    const humanId = humansByEmail.get(emailKey) ?? id();
    if (!humansByEmail.has(emailKey)) {
      statements.push({
        sql: `
          INSERT INTO humans (
            id, owner_user_id, name, email, created_at, updated_at, deleted_at
          )
          SELECT ?, ?, ?, ?, ?, ?, NULL
          WHERE EXISTS (
            SELECT 1 FROM sessions WHERE id = ? AND deleted_at IS NULL
          )
        `,
        params: [
          humanId,
          userId,
          participant.name || email,
          email,
          now,
          now,
          sessionId,
        ],
      });
    }

    statements.push({
      sql: `
        INSERT INTO session_participants (
          id, owner_user_id, session_id, human_id, display_name, email,
          source, created_at, updated_at, deleted_at
        )
        SELECT ?, ?, ?, ?, ?, ?, 'auto', ?, ?, NULL
        WHERE EXISTS (
          SELECT 1 FROM sessions WHERE id = ? AND deleted_at IS NULL
        )
          AND NOT EXISTS (
            SELECT 1
            FROM session_participants
            WHERE session_id = ? AND human_id = ? AND deleted_at IS NULL
          )
      `,
      params: [
        id(),
        userId,
        sessionId,
        humanId,
        participant.name || email,
        email,
        now,
        now,
        sessionId,
        sessionId,
        humanId,
      ],
    });
  }

  const rowsAffected = await executeTransaction(statements);

  const createdSessionId = await findSessionForEvent(event, sessionId);
  if (!createdSessionId) {
    throw new Error(`Failed to create a session for event ${eventId}`);
  }

  if (rowsAffected[0] === 1) {
    await trackNoteCreated(true);
  }
  return createdSessionId;
}

export async function softDeleteSession(
  sessionId: string,
): Promise<DeletedSessionData | null> {
  const [session] = await liveQueryClient.execute<SessionDeleteSqlRow>(
    `SELECT id, title FROM sessions WHERE id = ? AND deleted_at IS NULL LIMIT 1`,
    [sessionId],
  );
  if (!session) return null;

  const tombstone = new Date().toISOString();
  const rowsAffected = await executeTransaction(
    buildSessionTombstoneStatements(sessionId, tombstone),
  );
  if (rowsAffected[rowsAffected.length - 1] !== 1) return null;

  return {
    session: { id: session.id, title: session.title },
    tombstone,
    deletedAt: Date.now(),
  };
}

export async function isSessionEmpty(sessionId: string): Promise<boolean> {
  const [row] = await liveQueryClient.execute<SessionEmptySqlRow>(
    `
      SELECT
        sessions.title,
        sessions.event_json,
        COALESCE(note.body, '') AS note_body,
        COALESCE(note.body_format, '') AS note_body_format,
        (
          SELECT COUNT(*)
          FROM transcripts
          WHERE session_id = sessions.id AND deleted_at IS NULL
        ) AS transcript_count,
        (
          SELECT COUNT(*)
          FROM session_documents
          WHERE session_id = sessions.id
            AND kind IN ('summary', 'template_output')
            AND deleted_at IS NULL
        ) AS enhanced_note_count,
        (
          SELECT COUNT(*)
          FROM session_participants
          WHERE session_id = sessions.id
            AND source NOT IN ('auto', 'excluded')
            AND human_id <> sessions.owner_user_id
            AND deleted_at IS NULL
        ) AS manual_participant_count,
        (
          SELECT COUNT(*)
          FROM session_tags
          WHERE session_id = sessions.id AND deleted_at IS NULL
        ) AS tag_count
      FROM sessions
      LEFT JOIN session_documents AS note
        ON note.id = sessions.id
        AND note.kind = 'note'
        AND note.deleted_at IS NULL
      WHERE sessions.id = ? AND sessions.deleted_at IS NULL
      LIMIT 1
    `,
    [sessionId],
  );

  if (!row) return true;
  if (row.title.trim() && !row.event_json) return false;
  if (hasNoteContent(row.note_body, row.note_body_format)) return false;

  return (
    Number(row.transcript_count) === 0 &&
    Number(row.enhanced_note_count) === 0 &&
    Number(row.manual_participant_count) === 0 &&
    Number(row.tag_count) === 0
  );
}

export async function restoreDeletedSession(
  data: DeletedSessionData,
): Promise<void> {
  await executeTransaction(
    buildSessionTombstoneStatements(data.session.id, data.tombstone, true),
  );
}

/**
 * Reads back a trashed session's id, title and live `deleted_at` value so a
 * surface that only knows the session id (the Trash view) can build the
 * `DeletedSessionData` that `restoreDeletedSession` requires. Reading the
 * tombstone from the row (instead of assuming one) keeps the restore
 * predicate anchored to the exact tombstone the deletion actually wrote.
 */
export async function loadTrashedSessionData(
  sessionId: string,
): Promise<DeletedSessionData | null> {
  const [row] = await liveQueryClient.execute<{
    id: string;
    title: string;
    deleted_at: string;
  }>(
    `SELECT id, title, deleted_at FROM sessions WHERE id = ? AND deleted_at IS NOT NULL LIMIT 1`,
    [sessionId],
  );
  if (!row) return null;

  return {
    session: { id: row.id, title: row.title },
    tombstone: row.deleted_at,
    deletedAt: Date.parse(row.deleted_at) || 0,
  };
}

export async function finalizeSessionDeletion(
  sessionId: string,
): Promise<void> {
  try {
    const result = await fsSyncCommands.deleteSessionFolder(sessionId);
    if (result.status !== "error") return;
    console.error("[delete-session] failed to delete session folder", {
      sessionId,
      error: result.error,
    });
  } catch (error) {
    console.error("[delete-session] failed to delete session folder", {
      sessionId,
      error,
    });
  }
}

/**
 * Hard-DELETEs a tombstoned session and every owned child row, then removes
 * its folder from disk. The inverse of `softDeleteSession`: the same table
 * set, but as real DELETEs gated on the tombstone so a live row can never be
 * destroyed by mistake. Returns false when the session was not trashed (or
 * already purged), so callers can distinguish "nothing to do" from failure.
 */
export async function hardDeleteSession(sessionId: string): Promise<boolean> {
  const [session] = await liveQueryClient.execute<SessionDeleteSqlRow>(
    `SELECT id, title FROM sessions WHERE id = ? AND deleted_at IS NOT NULL LIMIT 1`,
    [sessionId],
  );
  if (!session) return false;

  await executeTransaction(buildSessionHardDeleteStatements(sessionId));
  await finalizeSessionDeletion(sessionId);
  return true;
}

/** "Empty trash": hard-DELETEs every currently-trashed session in one pass. */
export async function hardDeleteAllTrashedSessions(): Promise<number> {
  const rows = await liveQueryClient.execute<{ id: string }>(
    `SELECT id FROM sessions WHERE deleted_at IS NOT NULL`,
  );

  const statements = rows.flatMap((row) =>
    buildSessionHardDeleteStatements(row.id),
  );
  if (statements.length === 0) return 0;

  await executeTransaction(statements);
  await Promise.all(rows.map((row) => finalizeSessionDeletion(row.id)));
  return rows.length;
}

export function buildSessionTombstoneStatements(
  sessionId: string,
  tombstone: string,
  restore = false,
) {
  const value = restore ? null : tombstone;
  const predicate = restore ? "deleted_at = ?" : "deleted_at IS NULL";
  const predicateParams = restore ? [tombstone] : [];
  const directTables = SESSION_OWNED_TABLES;

  const statements = directTables.map((table) => ({
    sql: `
      UPDATE ${table}
      SET deleted_at = ?, updated_at = ?
      WHERE session_id = ? AND ${predicate}
    `,
    params: [value, tombstone, sessionId, ...predicateParams],
  }));

  statements.push({
    sql: `
      UPDATE entity_mentions
      SET deleted_at = ?, updated_at = ?
      WHERE (
        (source_type = 'session' AND source_id = ?)
        OR (target_type = 'session' AND target_id = ?)
      ) AND ${predicate}
    `,
    params: [value, tombstone, sessionId, sessionId, ...predicateParams],
  });
  statements.push({
    sql: `
      UPDATE sessions
      SET deleted_at = ?, updated_at = ?
      WHERE id = ? AND ${predicate}
    `,
    params: [value, tombstone, sessionId, ...predicateParams],
  });

  return statements;
}

/**
 * Hard-DELETE statements for a trashed session and its owned rows. Children
 * first, session row last, every statement gated on `deleted_at IS NOT NULL`
 * so only tombstoned data is ever physically removed.
 */
export function buildSessionHardDeleteStatements(sessionId: string) {
  const statements = SESSION_OWNED_TABLES.map((table) => ({
    sql: `DELETE FROM ${table} WHERE session_id = ? AND deleted_at IS NOT NULL`,
    params: [sessionId],
  }));

  statements.push({
    sql: `
      DELETE FROM entity_mentions
      WHERE (
        (source_type = 'session' AND source_id = ?)
        OR (target_type = 'session' AND target_id = ?)
      ) AND deleted_at IS NOT NULL
    `,
    params: [sessionId, sessionId],
  });
  statements.push({
    sql: `DELETE FROM sessions WHERE id = ? AND deleted_at IS NOT NULL`,
    params: [sessionId],
  });

  return statements;
}

function createEmptyNoteStatement(
  sessionId: string,
  userId: string,
  now: string,
  onlyIfSessionExists = false,
  body = "",
) {
  return {
    sql: `
      INSERT INTO session_documents (
        id, session_id, kind, body_format, body, created_by, updated_by,
        created_at, updated_at, deleted_at
      )
      ${onlyIfSessionExists ? "SELECT ?, ?, 'note', 'prosemirror_json', ?, ?, ?, ?, ?, NULL" : "VALUES (?, ?, 'note', 'prosemirror_json', ?, ?, ?, ?, ?, NULL)"}
      ${onlyIfSessionExists ? "WHERE EXISTS (SELECT 1 FROM sessions WHERE id = ? AND deleted_at IS NULL)" : ""}
    `,
    params: onlyIfSessionExists
      ? [sessionId, sessionId, body, userId, userId, now, now, sessionId]
      : [sessionId, sessionId, body, userId, userId, now, now],
  };
}

async function findSessionForEvent(
  event: EventSqlRow,
  preferredId?: string,
): Promise<string | null> {
  const rows = await liveQueryClient.execute<SessionIdentitySqlRow>(
    `
      SELECT id
      FROM sessions
      WHERE deleted_at IS NULL
        AND (event_id = ? OR (? <> '' AND external_event_id = ?))
      ORDER BY CASE WHEN id = ? THEN 0 ELSE 1 END, created_at, id
      LIMIT 1
    `,
    [
      event.id,
      event.tracking_id_event,
      event.tracking_id_event,
      preferredId ?? "",
    ],
  );
  return rows[0]?.id ?? null;
}

async function findHumansByEmail(
  participants: EventParticipant[],
): Promise<Map<string, string>> {
  const emails = Array.from(
    new Set(
      participants
        .map((participant) => participant.email?.trim().toLowerCase())
        .filter((email): email is string => Boolean(email)),
    ),
  );
  if (emails.length === 0) return new Map();

  const rows = await liveQueryClient.execute<HumanEmailSqlRow>(
    `
      SELECT id, email
      FROM humans
      WHERE deleted_at IS NULL
        AND lower(email) IN (${emails.map(() => "?").join(", ")})
      ORDER BY id
    `,
    emails,
  );
  return new Map(rows.map((row) => [row.email.toLowerCase(), row.id]));
}

function parseEventParticipants(value: string | null): EventParticipant[] {
  if (!value) return [];
  try {
    const parsed = JSON.parse(value) as unknown;
    return Array.isArray(parsed) ? (parsed as EventParticipant[]) : [];
  } catch {
    return [];
  }
}

function toSessionEvent(event: EventSqlRow): SessionEvent {
  return {
    tracking_id: event.tracking_id_event,
    calendar_id: event.calendar_id,
    title: event.title,
    started_at: event.started_at,
    ended_at: event.ended_at,
    is_all_day: Boolean(event.is_all_day),
    has_recurrence_rules: Boolean(event.has_recurrence_rules),
    location: event.location,
    meeting_link: event.meeting_link,
    description: event.description,
    recurrence_series_id: event.recurrence_series_id,
  };
}

function hasNoteContent(body: string, format: string): boolean {
  if (!body) return false;

  let markdown = body;
  if (format === "prosemirror_json") {
    try {
      markdown = json2md(JSON.parse(body));
    } catch {
      markdown = body;
    }
  }

  markdown = markdown.trim();
  return Boolean(markdown && markdown !== "&nbsp;");
}

function mapSessionRow(row: SessionSqlRow): SessionRecord {
  let rawMd = row.raw_body;
  if (rawMd && row.raw_body_format === "markdown") {
    try {
      rawMd = JSON.stringify(md2json(rawMd));
    } catch (error) {
      console.error("[session] failed to decode imported Markdown", error);
    }
  }

  return {
    id: row.id,
    user_id: row.owner_user_id,
    created_at: row.created_at,
    folder_id: row.folder_path,
    event_json: row.event_json,
    title: row.title,
    raw_md: rawMd,
  };
}

function mapSessionParticipantRow(
  row: SessionParticipantSqlRow,
): SessionParticipantRecord {
  return {
    id: row.id,
    sessionId: row.session_id,
    humanId: row.human_id,
    source: row.source,
    name: row.name,
    email: row.email,
    jobTitle: row.job_title,
    linkedinUsername: row.linkedin_username,
    organizationId: row.organization_id,
    organizationName: row.organization_name,
  };
}

function mapEnhancedNoteRow(row: EnhancedNoteSqlRow): EnhancedNoteRecord {
  let content = row.body;
  if (content && row.body_format === "markdown") {
    try {
      content = JSON.stringify(md2json(content));
    } catch (error) {
      console.error("[session] failed to decode summary Markdown", error);
    }
  }

  return {
    id: row.id,
    sessionId: row.session_id,
    title: row.title,
    content,
    templateId: row.template_id,
    position: Number(row.sort_order),
  };
}

async function trackNoteCreated(hasEventId: boolean): Promise<void> {
  try {
    await analyticsCommands.event({
      event: "note_created",
      has_event_id: hasEventId,
    });
  } catch (error) {
    console.error("[session] failed to record note creation analytics", error);
  }
}

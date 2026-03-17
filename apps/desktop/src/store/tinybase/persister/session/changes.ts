import { isValidTiptapContent, json2md } from "@hypr/tiptap/shared";

import {
  type ChangedTables,
  getChangedIds,
  SESSION_META_FILE,
  SESSION_NOTE_EXTENSION,
  SESSION_TRANSCRIPT_FILE,
  type TablesContent,
} from "~/store/tinybase/persister/shared";

export function parseSessionIdFromPath(path: string): string | null {
  const parts = path.split("/");
  const sessionsIndex = parts.indexOf("sessions");
  if (sessionsIndex === -1) {
    return null;
  }

  const filename = parts[parts.length - 1];
  const isSessionFile =
    filename === SESSION_META_FILE ||
    filename === SESSION_TRANSCRIPT_FILE ||
    filename?.endsWith(SESSION_NOTE_EXTENSION);

  if (isSessionFile && parts.length >= 2) {
    return parts[parts.length - 2] || null;
  }

  return null;
}

export type SessionChangeResult = {
  changedSessionIds: Set<string>;
  emptySessionIds: Set<string>;
  hasUnresolvedDeletions: boolean;
};

function isSessionEmpty(tables: TablesContent, sessionId: string): boolean {
  const session = tables.sessions?.[sessionId];
  if (!session) {
    return true;
  }

  if (session.title && session.title.trim() && !session.event_json) {
    return false;
  }

  if (session.raw_md) {
    let rawMd: string;
    try {
      const parsed = JSON.parse(session.raw_md);
      rawMd = isValidTiptapContent(parsed) ? json2md(parsed) : session.raw_md;
    } catch {
      rawMd = session.raw_md;
    }
    rawMd = rawMd.trim();
    if (rawMd && rawMd !== "&nbsp;") {
      return false;
    }
  }

  const transcripts = tables.transcripts ?? {};
  for (const row of Object.values(transcripts)) {
    if (row.session_id === sessionId) {
      return false;
    }
  }

  const enhancedNotes = tables.enhanced_notes ?? {};
  for (const row of Object.values(enhancedNotes)) {
    if (row.session_id === sessionId) {
      return false;
    }
  }

  const participants = tables.mapping_session_participant ?? {};
  for (const row of Object.values(participants)) {
    if (row.session_id === sessionId && row.source !== "auto") {
      return false;
    }
  }

  const tagMappings = tables.mapping_tag_session ?? {};
  for (const row of Object.values(tagMappings)) {
    if (row.session_id === sessionId) {
      return false;
    }
  }

  return true;
}

export function getChangedSessionIds(
  tables: TablesContent,
  changedTables: ChangedTables,
): SessionChangeResult | undefined {
  const result = getChangedIds(tables, changedTables, [
    { table: "sessions", extractId: (id) => id },
    {
      table: "mapping_session_participant",
      extractId: (id, tables) =>
        tables.mapping_session_participant?.[id]?.session_id,
    },
    {
      table: "transcripts",
      extractId: (id, tables) => tables.transcripts?.[id]?.session_id,
    },
    {
      table: "enhanced_notes",
      extractId: (id, tables) => tables.enhanced_notes?.[id]?.session_id,
    },
  ]);

  if (!result) {
    return undefined;
  }

  const changedSessionIds = new Set<string>();
  const emptySessionIds = new Set<string>();

  for (const id of result.changedIds) {
    if (isSessionEmpty(tables, id)) {
      emptySessionIds.add(id);
    } else {
      changedSessionIds.add(id);
    }
  }

  if (
    changedSessionIds.size === 0 &&
    emptySessionIds.size === 0 &&
    !result.hasUnresolvedDeletions
  ) {
    return undefined;
  }

  return {
    changedSessionIds,
    emptySessionIds,
    hasUnresolvedDeletions: result.hasUnresolvedDeletions,
  };
}

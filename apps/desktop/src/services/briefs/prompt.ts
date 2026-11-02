import type {
  AttendeeInfo,
  BriefCandidateEvent,
  BriefRetrievalContext,
  OpenActionItem,
  RelevantMeeting,
} from "./types";

import { parseEventParticipants } from "~/calendar/queries";
import { BUILTIN_RECIPES } from "~/chat/recipes/defaults";
import { searchMeetingContent } from "~/chat/tools/note-files";
import { liveQueryClient } from "~/db";
import {
  loadSessionContentSnapshot,
  type SessionContentSnapshot,
} from "~/session/content-queries";

const CONSUMER_EMAIL_DOMAINS: Record<string, true> = {
  "gmail.com": true,
  "googlemail.com": true,
  "yahoo.com": true,
  "hotmail.com": true,
  "outlook.com": true,
  "icloud.com": true,
  "me.com": true,
  "mac.com": true,
  "proton.me": true,
  "protonmail.com": true,
  "aol.com": true,
};

function extractDomainOrg(email: string | undefined): string | undefined {
  if (!email || !email.includes("@")) return undefined;
  const domain = email.split("@")[1]?.toLowerCase().trim();
  if (!domain || CONSUMER_EMAIL_DOMAINS[domain]) return undefined;
  const parts = domain.split(".");
  if (parts.length >= 2) {
    const main = parts[parts.length - 2];
    if (main && main.length > 1) {
      return main.charAt(0).toUpperCase() + main.slice(1);
    }
  }
  return undefined;
}

export async function resolveAttendees(
  participantsJson: string | null | undefined,
): Promise<AttendeeInfo[]> {
  const participants = parseEventParticipants(
    participantsJson ?? undefined,
  ).filter((p) => p.is_current_user !== true);

  if (participants.length === 0) {
    return [];
  }

  const emails = participants
    .map((p) => p.email?.trim().toLowerCase())
    .filter((e): e is string => Boolean(e));
  const names = participants
    .map((p) => p.name?.trim())
    .filter((n): n is string => Boolean(n));

  const humanOrgMap = new Map<string, string>();

  if (emails.length > 0 || names.length > 0) {
    const conditions: string[] = [];
    const params: string[] = [];

    if (emails.length > 0) {
      conditions.push(
        `lower(human.email) IN (${emails.map(() => "?").join(",")})`,
      );
      params.push(...emails);
    }
    if (names.length > 0) {
      conditions.push(
        `lower(human.name) IN (${names.map(() => "?").join(",")})`,
      );
      params.push(...names.map((n) => n.toLowerCase()));
    }

    const rows = await liveQueryClient.execute<{
      email: string;
      name: string;
      org_name: string | null;
    }>(
      `
        SELECT
          human.email,
          human.name,
          org.name AS org_name
        FROM humans AS human
        LEFT JOIN organizations AS org
          ON org.id = human.organization_id AND org.deleted_at IS NULL
        WHERE human.deleted_at IS NULL
          AND (${conditions.join(" OR ")})
      `,
      params,
    );

    for (const row of rows) {
      if (row.org_name) {
        if (row.email) humanOrgMap.set(row.email.toLowerCase(), row.org_name);
        if (row.name) humanOrgMap.set(row.name.toLowerCase(), row.org_name);
      }
    }
  }

  return participants.map((p) => {
    const name = p.name?.trim() || p.email?.trim() || "Unknown";
    const email = p.email?.trim() || undefined;
    const org =
      (email ? humanOrgMap.get(email.toLowerCase()) : undefined) ??
      humanOrgMap.get(name.toLowerCase()) ??
      extractDomainOrg(email);

    return { name, email, org };
  });
}

export async function retrieveBriefContext(
  event: BriefCandidateEvent,
): Promise<BriefRetrievalContext> {
  const attendees = await resolveAttendees(event.participants_json);
  const candidateSessionIds = new Set<string>();
  const sessionTimestamps = new Map<string, number>();

  // 1. Same series history
  if (event.recurrence_series_id) {
    try {
      const seriesRows = await liveQueryClient.execute<{ id: string }>(
        `
          SELECT id
          FROM sessions
          WHERE series_id = ? AND deleted_at IS NULL
          ORDER BY COALESCE(NULLIF(started_at, ''), created_at) DESC
          LIMIT 5
        `,
        [event.recurrence_series_id],
      );
      for (const r of seriesRows) {
        candidateSessionIds.add(r.id);
      }
    } catch (error) {
      console.warn("[briefs] failed to load recurrence series sessions", error);
    }
  }

  // 2. Meetings with the same attendees
  for (const attendee of attendees) {
    if (candidateSessionIds.size >= 10) break;
    try {
      const searchRes = await searchMeetingContent({
        query: attendee.name,
        limit: 3,
      });
      for (const match of searchRes.results) {
        candidateSessionIds.add(match.sessionId);
        const startMs = match.snippets[0]?.startMs;
        if (
          typeof startMs === "number" &&
          !sessionTimestamps.has(match.sessionId)
        ) {
          sessionTimestamps.set(match.sessionId, startMs);
        }
      }
    } catch (error) {
      console.warn(
        `[briefs] failed to search meetings for ${attendee.name}`,
        error,
      );
    }
  }

  // 3. Mentions of attendees' org
  const distinctOrgs = Array.from(
    new Set(attendees.map((a) => a.org).filter((o): o is string => Boolean(o))),
  );
  const orgMentions: Array<{
    sessionId: string;
    title: string;
    snippet: string;
  }> = [];

  for (const org of distinctOrgs) {
    try {
      const orgRes = await searchMeetingContent({
        query: org,
        limit: 3,
      });
      for (const match of orgRes.results) {
        candidateSessionIds.add(match.sessionId);
        const startMs = match.snippets[0]?.startMs;
        if (
          typeof startMs === "number" &&
          !sessionTimestamps.has(match.sessionId)
        ) {
          sessionTimestamps.set(match.sessionId, startMs);
        }
        if (match.snippets[0]) {
          orgMentions.push({
            sessionId: match.sessionId,
            title: match.title,
            snippet: match.snippets[0].text,
          });
        }
      }
    } catch (error) {
      console.warn(`[briefs] failed to search org mentions for ${org}`, error);
    }
  }

  // Load session snapshots for top candidate sessions
  const relevantMeetings: RelevantMeeting[] = [];
  const openActionItems: OpenActionItem[] = [];

  for (const sessionId of candidateSessionIds) {
    if (relevantMeetings.length >= 5) break;

    try {
      const snapshot = await loadSessionContentSnapshot(sessionId);
      if (!snapshot) continue;

      const outcome = extractMeetingOutcome(snapshot);
      const timestampMs = sessionTimestamps.get(sessionId) ?? 0;

      relevantMeetings.push({
        sessionId,
        title: snapshot.title.trim() || "Untitled meeting",
        date: snapshot.createdAt,
        outcome,
        timestampMs,
      });

      for (const item of snapshot.actionItems) {
        if (item.status !== "done") {
          openActionItems.push({
            text: item.text,
            dueAt: item.dueAt || undefined,
            sourceSessionId: sessionId,
            timestampMs: 0,
          });
        }
      }
    } catch (error) {
      console.warn(
        `[briefs] failed to load snapshot for session ${sessionId}`,
        error,
      );
    }
  }

  return {
    attendees,
    relevantMeetings: relevantMeetings.slice(0, 3),
    openActionItems: openActionItems.slice(0, 5),
    orgMentions: orgMentions.slice(0, 3),
  };
}

function extractMeetingOutcome(snapshot: SessionContentSnapshot): string {
  if (
    snapshot.enhancedNotes.length > 0 &&
    snapshot.enhancedNotes[0]?.markdown
  ) {
    const lines = snapshot.enhancedNotes[0].markdown
      .split("\n")
      .map((l) => l.trim())
      .filter((l) => l.length > 0 && !l.startsWith("#"));
    if (lines[0]) {
      return lines[0].slice(0, 200);
    }
  }

  if (snapshot.rawMarkdown) {
    const lines = snapshot.rawMarkdown
      .split("\n")
      .map((l) => l.trim())
      .filter((l) => l.length > 0 && !l.startsWith("#"));
    if (lines[0]) {
      return lines[0].slice(0, 200);
    }
  }

  return "Meeting concluded with recorded notes.";
}

export function assembleBriefPrompt(
  event: BriefCandidateEvent,
  context: BriefRetrievalContext,
): { system: string; prompt: string } {
  const recipe = BUILTIN_RECIPES.find(
    (r) => r.id === "builtin-pre-meeting-brief",
  );
  const basePrompt = recipe?.prompt ?? "Generate a concise pre-meeting brief.";

  const system = `You are Notare's executive briefing assistant.
Generate a concise, high-value pre-meeting brief for the user before their upcoming meeting.

STRUCTURE REQUIREMENTS:
1. **Attendees**: List each participant with their name and organization (e.g. "- Jane Doe (Acme Corp)").
2. **Previous Meetings**: Last relevant meetings (up to 3) with a single-line outcome each. MUST cite the source meeting using the exact citation format: \`[[src:<sessionId>@<ms>]]\`.
3. **Open Action Items**: Incomplete action items involving these attendees or relevant topics, cited with \`[[src:<sessionId>@<ms>]]\`. If none, state "No open action items".
4. **Suggested Questions**: 2-3 strategic, insightful questions to ask during this meeting.

CITATION FORMAT RULES:
- Use ONLY the exact \`[[src:<sessionId>@<ms>]]\` tags provided in the context.
- Never invent, fabricate, or hallucinate session IDs.
- Keep the brief tight, scannable, and formatted in clean Markdown.`;

  const attendeesText =
    context.attendees.length > 0
      ? context.attendees
          .map(
            (a) =>
              `- ${a.name}${a.org ? ` (${a.org})` : ""}${a.email ? ` <${a.email}>` : ""}`,
          )
          .join("\n")
      : "No external attendees listed.";

  const meetingsText =
    context.relevantMeetings.length > 0
      ? context.relevantMeetings
          .map(
            (m) =>
              `- [[src:${m.sessionId}@${m.timestampMs}]] "${m.title}" (${m.date ? new Date(m.date).toLocaleDateString() : "Recent"}): ${m.outcome}`,
          )
          .join("\n")
      : "No previous related meetings found.";

  const actionItemsText =
    context.openActionItems.length > 0
      ? context.openActionItems
          .map((ai) => {
            const timestamp = ai.timestampMs ?? 0;
            const cite = ai.sourceSessionId
              ? ` [[src:${ai.sourceSessionId}@${timestamp}]]`
              : "";
            const due = ai.dueAt ? ` (Due: ${ai.dueAt})` : "";
            return `- ${ai.text}${due}${cite}`;
          })
          .join("\n")
      : "No outstanding action items.";

  const orgMentionsText =
    context.orgMentions.length > 0
      ? context.orgMentions
          .map(
            (om) => `- [[src:${om.sessionId}@0]] ${om.title}: "${om.snippet}"`,
          )
          .join("\n")
      : "";

  const prompt = `UPCOMING MEETING:
Title: ${event.title || "Untitled Meeting"}
Time: ${event.started_at} - ${event.ended_at}
${event.location ? `Location: ${event.location}\n` : ""}${event.meeting_link ? `Link: ${event.meeting_link}\n` : ""}${event.description ? `Description: ${event.description}\n` : ""}
ATTENDEES:
${attendeesText}

PAST RELEVANT MEETINGS:
${meetingsText}

OPEN ACTION ITEMS:
${actionItemsText}
${orgMentionsText ? `\nORGANIZATION MENTIONS:\n${orgMentionsText}\n` : ""}
INSTRUCTION:
${basePrompt.replace("{{selection}}", "").trim()}`;

  return { system, prompt };
}

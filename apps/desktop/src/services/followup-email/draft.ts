export interface AssembleDraftInput {
  sessionTitle?: string | null;
  notesContent?: string | null;
  transcriptText?: string | null;
  attendees?: string[] | null;
}

export interface FollowupDraft {
  subject: string;
  body: string;
}

export const FOLLOWUP_SYSTEM_PROMPT = `You are an executive assistant preparing a follow-up email after a meeting.
Based on the provided meeting notes and context, generate a concise, professional follow-up email.
Your output must include:
1. Recap: Brief summary of what was discussed.
2. Decisions: Key agreements reached.
3. Action items: Concrete tasks explicitly assigned to specific owners with deadlines if mentioned.
4. Next steps: Follow-up meetings or milestones.

Format your response exactly as follows:
SUBJECT: <Concise, clear subject line>
BODY:
<The full email body in clear, professional plain text>`;

export function assembleFollowupPrompt(input: AssembleDraftInput): string {
  const parts: string[] = [];

  if (input.sessionTitle) {
    parts.push(`Meeting: ${input.sessionTitle}`);
  }

  if (input.attendees && input.attendees.length > 0) {
    parts.push(`Attendees: ${input.attendees.join(", ")}`);
  }

  if (input.notesContent && input.notesContent.trim().length > 0) {
    parts.push(`--- Meeting Notes ---\n${input.notesContent.trim()}`);
  }

  if (input.transcriptText && input.transcriptText.trim().length > 0) {
    parts.push(`--- Transcript Excerpt ---\n${input.transcriptText.trim()}`);
  }

  return parts.join("\n\n");
}

export function parseFollowupOutput(
  rawOutput: string,
  defaultSubject = "Meeting Follow-up",
): FollowupDraft {
  const trimmed = rawOutput.trim();

  let subject = defaultSubject;
  let body = trimmed;

  const subjectMatch = trimmed.match(/^SUBJECT:\s*(.+)$/im);
  if (subjectMatch) {
    subject = subjectMatch[1].trim();
  }

  const bodyMatch = trimmed.match(/^BODY:\s*([\s\S]*)$/im);
  if (bodyMatch) {
    body = bodyMatch[1].trim();
  } else if (subjectMatch) {
    // If SUBJECT was found but no explicit BODY marker, take the rest after the SUBJECT line
    const afterSubject = trimmed.replace(/^SUBJECT:\s*.+$/im, "").trim();
    if (afterSubject) {
      body = afterSubject;
    }
  }

  return { subject, body };
}

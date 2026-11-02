import type { Recipe } from "@hypr/plugin-db";

export const BUILTIN_RECIPES: ReadonlyArray<
  Omit<Recipe, "createdAt" | "updatedAt">
> = [
  {
    id: "builtin-action-items",
    title: "Action items",
    prompt:
      "Identify all action items, tasks, and follow-ups mentioned in this meeting. For each item, include:\n- Description of the task\n- Owner / assignee (if mentioned)\n- Due date or timeframe (if mentioned)\n\n{{selection}}",
    scope: "single",
    builtin: true,
  },
  {
    id: "builtin-key-decisions",
    title: "Key decisions",
    prompt:
      "Summarize the key decisions made during this meeting, along with the context, rationale, and any alternatives or trade-offs that were discussed.\n\n{{selection}}",
    scope: "single",
    builtin: true,
  },
  {
    id: "builtin-follow-up-email",
    title: "Follow-up email draft",
    prompt:
      "Draft a concise, professional follow-up email summarizing the main discussion points, decisions made, and agreed next steps from this meeting.\n\n{{selection}}",
    scope: "single",
    builtin: true,
  },
  {
    id: "builtin-customer-objections",
    title: "Customer objections",
    prompt:
      "List all customer objections, pain points, hesitations, or concerns raised during the conversation, along with how they were addressed and recommended next steps.\n\n{{selection}}",
    scope: "single",
    builtin: true,
  },
  {
    id: "builtin-weekly-digest",
    title: "Weekly digest",
    prompt:
      "Synthesize a comprehensive weekly digest across the selected meetings. Highlight the major themes, progress updates, critical decisions, blockers, and overarching priorities.\n\n{{selection}}",
    scope: "multi",
    builtin: true,
  },
  {
    id: "builtin-questions-not-asked",
    title: "Questions I didn't ask",
    prompt:
      "Analyze the meeting discussion and identify important questions, unexamined assumptions, risks, or follow-ups that were not addressed but should be considered.\n\n{{selection}}",
    scope: "single",
    builtin: true,
  },
  {
    id: "builtin-pre-meeting-brief",
    title: "Pre-meeting brief",
    prompt:
      "Generate a concise pre-meeting brief for the upcoming meeting:\n" +
      "- Attendees: Who is attending (names and organization)\n" +
      "- Recent Meetings: Last 3 relevant meetings with one-line outcomes (cite sources with [[src:<sessionId>@<ms>]])\n" +
      "- Open Action Items: Outstanding action items involving these attendees\n" +
      "- Suggested Questions: Strategic questions to ask or discuss\n\n" +
      "{{selection}}",
    scope: "single",
    builtin: true,
  },
];

export type SessionBrief = {
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

export type BriefCandidateEvent = {
  id: string;
  title: string;
  started_at: string;
  ended_at: string;
  location?: string | null;
  meeting_link?: string | null;
  description?: string | null;
  calendar_id?: string | null;
  recurrence_series_id?: string | null;
  participants_json?: string | null;
  is_all_day?: number | boolean;
};

export type AttendeeInfo = {
  name: string;
  email?: string;
  org?: string;
};

export type RelevantMeeting = {
  sessionId: string;
  title: string;
  date: string | null;
  outcome: string;
  timestampMs: number;
};

export type OpenActionItem = {
  text: string;
  assignee?: string;
  dueAt?: string;
  sourceSessionId?: string;
  timestampMs?: number;
};

export type BriefRetrievalContext = {
  attendees: AttendeeInfo[];
  relevantMeetings: RelevantMeeting[];
  openActionItems: OpenActionItem[];
  orgMentions: Array<{ sessionId: string; title: string; snippet: string }>;
};

export type GenerateBriefOptions = {
  signal?: AbortSignal;
};

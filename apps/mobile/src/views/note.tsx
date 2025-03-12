import type { ActivityLoaderArgs } from "@stackflow/config";
import { AppScreen } from "@stackflow/plugin-basic-ui";
import { ActivityComponentType, useLoaderData } from "@stackflow/react/future";
import { NoteContent, NoteInfo } from "../components/note";
import { mockSessions } from "../mock/home";

export function noteActivityLoader({
  params,
}: ActivityLoaderArgs<"NoteActivity">) {
  const { id } = params;

  // Find the session in the mock data or return a default session
  const session = mockSessions.find(s => s.id === id) || {
    id,
    title: "Untitled Note",
    created_at: new Date().toISOString(),
    visited_at: new Date().toISOString(),
    user_id: "user-123",
    calendar_event_id: null,
    audio_local_path: null,
    audio_remote_path: null,
    raw_memo_html: "<p>No content available.</p>",
    enhanced_memo_html: "<h2>Untitled Note</h2><p>No content available.</p>",
    conversations: [],
  };

  return { session };
}

export const NoteActivity: ActivityComponentType<"NoteActivity"> = () => {
  const { session } = useLoaderData<typeof noteActivityLoader>();

  return (
    <AppScreen
      appBar={{
        title: "Note - " + session.id,
      }}
    >
      <div className="relative flex h-full flex-col">
        <div className="flex-1 overflow-y-auto py-6">
          <NoteInfo session={session} />
          <NoteContent session={session} />
        </div>
      </div>
    </AppScreen>
  );
};

declare module "@stackflow/config" {
  interface Register {
    NoteActivity: {
      id: string;
    };
  }
}

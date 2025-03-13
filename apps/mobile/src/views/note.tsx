import type { ActivityLoaderArgs } from "@stackflow/config";
import { AppScreen } from "@stackflow/plugin-basic-ui";
import { ActivityComponentType, useLoaderData } from "@stackflow/react/future";
import { Share2Icon } from "lucide-react";
import { useNote } from "../components/hooks/use-note";
import { NoteContent, NoteInfo } from "../components/note";
import { ShareSheet } from "../components/note/bottom-sheets";
import { mockSessions } from "../mock/home";

export function noteLoader({
  params,
}: ActivityLoaderArgs<"NoteView">) {
  const { id } = params;

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

export const NoteView: ActivityComponentType<"NoteView"> = () => {
  const { session } = useLoaderData<typeof noteLoader>();
  const {
    shareSheetOpen,
    setShareSheetOpen,
    handlePublishNote,
  } = useNote({ session });

  const ShareButton = () => (
    <button onClick={() => setShareSheetOpen(true)}>
      <Share2Icon size={20} />
    </button>
  );

  return (
    <AppScreen
      appBar={{
        renderRight: ShareButton,
      }}
    >
      <div className="relative flex h-full flex-col">
        <div className="flex-1 overflow-y-auto py-6">
          <NoteInfo session={session} />
          <NoteContent session={session} />
        </div>

        <ShareSheet
          open={shareSheetOpen}
          onClose={() => setShareSheetOpen(false)}
          onPublish={handlePublishNote}
        />
      </div>
    </AppScreen>
  );
};

declare module "@stackflow/config" {
  interface Register {
    NoteView: {
      id: string;
    };
  }
}

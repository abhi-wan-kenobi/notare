import type { ActivityLoaderArgs } from "@stackflow/config";
import { AppScreen } from "@stackflow/plugin-basic-ui";
import { ActivityComponentType, useLoaderData } from "@stackflow/react/future";
import { GlobeIcon, Share2Icon } from "lucide-react";
import * as React from "react";

import { NoteContent, NoteInfo } from "../components/note";
import { mockSessions } from "../mock/home";

import { BottomSheet, BottomSheetContent } from "@hypr/ui/components/ui/bottom-sheet";
import { Button } from "@hypr/ui/components/ui/button";

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
  const [shareSheetOpen, setShareSheetOpen] = React.useState(false);

  const handleShareNote = () => {
    setShareSheetOpen(true);
  };

  const ShareButton = () => (
    <button onClick={handleShareNote}>
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

        <BottomSheet
          open={shareSheetOpen}
          onClose={() => setShareSheetOpen(false)}
        >
          <BottomSheetContent className="bg-white">
            <div className="flex flex-col gap-4 p-4">
              <div className="text-center">
                <h3 className="text-lg font-medium mb-1">Publish your note</h3>
                <p className="text-sm text-neutral-600">
                  Anyone with the link can view this page
                </p>
              </div>

              <Button size="lg">
                <GlobeIcon className="size-4 mr-2" /> Make it public
              </Button>
            </div>
          </BottomSheetContent>
        </BottomSheet>
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

import { useMutation, useQueryClient } from "@tanstack/react-query";
import { createFileRoute } from "@tanstack/react-router";
import { useEffect } from "react";

import EditorArea from "@/components/note/editor-area";
import { useSession } from "@/contexts";
import { commands as dbCommands } from "@hypr/plugin-db";

export const Route = createFileRoute("/app/note/$id/main")({
  component: Component,
});

function Component() {
  const { sessionId, getSession } = useSession((s) => ({
    sessionId: s.session.id,
    getSession: s.getSession,
  }));

  const queryClient = useQueryClient();

  const mutation = useMutation({
    mutationKey: ["delete-session", sessionId],
    mutationFn: () => dbCommands.deleteSession(sessionId),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["sessions"] });
    },
    onError: (error) => {
      console.error(error);
    },
  });

  useEffect(() => {
    const isEmpty = (s: string | null) => s === "<p></p>" || !s;

    return () => {
      const session = getSession();

      const isNoteEmpty = !session.title
        && isEmpty(session.raw_memo_html)
        && isEmpty(session.enhanced_memo_html)
        && session.conversations.length === 0;

      if (isNoteEmpty) {
        mutation.mutate();
      }
    };
  }, [getSession]);

  return (
    <main className="flex h-full overflow-hidden bg-white">
      <div className="h-full flex-1 pt-6">
        <EditorArea editable={true} />
      </div>
    </main>
  );
}

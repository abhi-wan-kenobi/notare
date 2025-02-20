import { createFileRoute, redirect } from "@tanstack/react-router";
import { commands as dbCommands } from "@hypr/plugin-db";

import { type Config } from "@/types";
import { SessionProvider } from "@/contexts";
import EditorArea from "@/components/note/editor";
import NoteAIButton from "@/components/note-ai-button";
function Component() {
  const { session } = Route.useLoaderData();

  return (
    <SessionProvider session={session}>
      <div className="relative flex h-full flex-col overflow-hidden">
        <EditorArea />
        <NoteAIButton />
      </div>
    </SessionProvider>
  );
}

export const Route = createFileRoute("/_nav/note/$id")({
  component: Component,
  loader: ({ context: { queryClient }, params: { id } }) => {
    return queryClient.fetchQuery({
      queryKey: ["note", { id }],
      queryFn: async () => {
        const [session, config, customTemplates] = await Promise.all([
          dbCommands.getSession({ id }),
          dbCommands.getConfig(),
          dbCommands.listTemplates(),
        ]);
        if (!session) {
          throw redirect({ to: "/" });
        }

        return {
          session,
          config: config as Config,
          templates: [...customTemplates],
        };
      },
    });
  },
});

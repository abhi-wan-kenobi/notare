import { createFileRoute, redirect } from "@tanstack/react-router";
import { commands as dbCommands, type Config } from "@hypr/plugin-db";

import { SessionProvider } from "@/contexts";
import EditorArea from "@/components/note/editor";
import RightPanel from "@/components/note/right-panel";

function Component() {
  const { session } = Route.useLoaderData();

  return (
    <SessionProvider session={session}>
      <div className="flex h-full w-full">
        <div className="relative flex h-full flex-1 flex-col overflow-hidden">
          <EditorArea />
        </div>

        <RightPanel />
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

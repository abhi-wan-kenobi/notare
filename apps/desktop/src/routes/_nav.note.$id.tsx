import { createFileRoute, redirect } from "@tanstack/react-router";
import { commands as dbCommands, type Config } from "@hypr/plugin-db";

import { SessionProvider } from "@/contexts";
import EditorArea from "@/components/note/editor";
import {
  ResizableHandle,
  ResizablePanel,
  ResizablePanelGroup,
} from "@hypr/ui/components/ui/resizable";
import RightPanel from "@/components/note/right-panel";

function Component() {
  const { session } = Route.useLoaderData();

  return (
    <SessionProvider session={session}>
      <ResizablePanelGroup direction="horizontal">
        <ResizablePanel>
          <div className="relative flex h-full flex-col overflow-hidden">
            <EditorArea />
          </div>
        </ResizablePanel>
        <ResizableHandle withHandle />
        <ResizablePanel defaultSize={30} minSize={30} maxSize={60}>
          <RightPanel />
        </ResizablePanel>
      </ResizablePanelGroup>
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

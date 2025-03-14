import { createFileRoute } from "@tanstack/react-router";

import type { RoutePath } from "@/types";
import { commands as dbCommands } from "@hypr/plugin-db";
import { commands as windowsCommands } from "@hypr/plugin-windows";

export const Route = createFileRoute("/app/calendar")({
  component: RouteComponent,
  loader: async ({ context: { queryClient } }) => {
    const sessions = await queryClient.fetchQuery({
      queryKey: ["sessions"],
      queryFn: () => dbCommands.listSessions(null),
    });

    return { sessions };
  },
});

function RouteComponent() {
  const { sessions } = Route.useLoaderData();

  const handleClick = (id: string) => {
    const path: RoutePath = "/app/note/$id/main";
    windowsCommands.windowNavigate("main", path.replace("$id", id));
  };

  return (
    <div className="flex min-h-screen items-center justify-center">
      <ul className="flex flex-col gap-2">
        {sessions.map((session) => (
          <li key={session.id}>
            <button
              className="p-1 rounded-md bg-neutral-300 hover:bg-neutral-400"
              onClick={() => handleClick(session.id)}
            >
              {`Move to ${session.id}`}
            </button>
          </li>
        ))}
      </ul>
    </div>
  );
}

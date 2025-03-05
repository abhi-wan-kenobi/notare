import { createFileRoute, redirect } from "@tanstack/react-router";

import PastSessions from "@/components/past-sessions";
import UpcomingEvents from "@/components/upcoming-events";

import { commands as authCommands } from "@hypr/plugin-auth";

export const Route = createFileRoute("/app/")({
  component: Component,
  beforeLoad: async () => {
    if (!import.meta.env.PROD) {
      return;
    }

    const id = await authCommands.getFromStore("auth-user-id");
    if (!id) {
      throw redirect({ to: "/login" });
    }
  },
});

function Component() {
  return (
    <main className="flex h-full flex-col overflow-hidden bg-white">
      <div className="overflow-y-auto px-8">
        <div className="mx-auto max-w-3xl">
          <UpcomingEvents />
          <PastSessions />
        </div>
      </div>
    </main>
  );
}

import type { ActivityLoaderArgs } from "@stackflow/config";
import { AppScreen } from "@stackflow/plugin-basic-ui";
import { ActivityComponentType, useFlow, useLoaderData } from "@stackflow/react/future";
import { useQuery } from "@tanstack/react-query";
import { clsx } from "clsx";
import { SquarePen } from "lucide-react";

import { commands as dbCommands } from "@hypr/plugin-db";

export function homeActivityLoader({}: ActivityLoaderArgs<"HomeActivity">) {
  const sessions = [
    {
      id: "1",
      name: "Session 1",
    },
  ];
  return {
    sessions,
  };
}

export const HomeActivity: ActivityComponentType<"HomeActivity"> = () => {
  const { sessions } = useLoaderData<typeof homeActivityLoader>();
  const { push } = useFlow();

  useQuery({
    queryKey: ["sessions"],
    queryFn: () => dbCommands.listSessions(null),
  });

  const handleClickNote = (id: string) => {
    push("SessionActivity", { id });
  };

  const handleClickNew = () => {
    push("SessionActivity", { id: "new" });
  };

  const RightButton = () => (
    <button onClick={handleClickNew}>
      <SquarePen size={24} />
    </button>
  );

  return (
    <AppScreen
      appBar={{
        title: "All Notes",
        renderRight: () => <RightButton />,
      }}
    >
      <div>
        {sessions.map((session) => (
          <div
            key={session.id}
            onClick={() => handleClickNote(session.id)}
            className={clsx([
              "p-4 border  rounded-md",
              "border-gray-200 hover:bg-gray-300",
            ])}
          >
            <h1>{session.name}</h1>
          </div>
        ))}
      </div>
    </AppScreen>
  );
};

declare module "@stackflow/config" {
  interface Register {
    HomeActivity: {};
  }
}

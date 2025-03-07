import {
  ActivityComponentType,
  useFlow,
  useLoaderData,
} from "@stackflow/react/future";
import { AppScreen } from "@stackflow/plugin-basic-ui";
import type { ActivityLoaderArgs } from "@stackflow/config";

export function homeActivityLoader({}: ActivityLoaderArgs<"HomeActivity">) {
  return {
    sessions: [
      {
        id: "TODO",
        name: "TODO",
      },
    ],
  };
}

export const HomeActivity: ActivityComponentType<"HomeActivity"> = () => {
  const { sessions } = useLoaderData<typeof homeActivityLoader>();
  const { push } = useFlow();

  const handleClick = (id: string) => {
    push("SessionActivity", { id });
  };

  return (
    <AppScreen appBar={{ title: "Hyprnote" }}>
      <div>
        {sessions.map((session) => (
          <div
            className="p-4 border border-gray-200 rounded-md hover:bg-gray-300 cursor-pointer"
            key={session.id}
            onClick={() => handleClick(session.id)}
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

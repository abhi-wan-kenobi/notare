import { ActivityComponentType, useLoaderData } from "@stackflow/react/future";
import { AppScreen } from "@stackflow/plugin-basic-ui";
import type { ActivityLoaderArgs } from "@stackflow/config";

export function sessionActivityLoader({
  params,
}: ActivityLoaderArgs<"SessionActivity">) {
  const { id } = params;

  const session = { id };

  return { session };
}

export const SessionActivity: ActivityComponentType<"SessionActivity"> = () => {
  const { session } = useLoaderData<typeof sessionActivityLoader>();

  return (
    <AppScreen appBar={{ title: "Hyprnote - Note" }}>
      <div>
        <h1>NoteActivity</h1>
        <p>{JSON.stringify(session)}</p>
      </div>
    </AppScreen>
  );
};

declare module "@stackflow/config" {
  interface Register {
    SessionActivity: {
      id: string;
    };
  }
}

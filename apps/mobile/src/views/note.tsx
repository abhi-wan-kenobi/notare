import type { ActivityLoaderArgs } from "@stackflow/config";
import { AppScreen } from "@stackflow/plugin-basic-ui";
import { ActivityComponentType, useLoaderData } from "@stackflow/react/future";

export function noteActivityLoader({
  params,
}: ActivityLoaderArgs<"NoteActivity">) {
  const { id } = params;

  const session = { id };

  return { session };
}

export const NoteActivity: ActivityComponentType<"NoteActivity"> = () => {
  const { session } = useLoaderData<typeof noteActivityLoader>();

  return (
    <AppScreen
      appBar={{
        title: "Note - " + session.id,
      }}
    >
      <div>
        <h1>NoteActivity</h1>
        <p>{JSON.stringify(session)}</p>
      </div>
    </AppScreen>
  );
};

declare module "@stackflow/config" {
  interface Register {
    NoteActivity: {
      id: string;
    };
  }
}

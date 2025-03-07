import { ActivityComponentType, useLoaderData } from "@stackflow/react/future";
import { AppScreen } from "@stackflow/plugin-basic-ui";

import { noteActivityLoader } from "./note.loader";

declare module "@stackflow/config" {
  interface Register {
    NoteActivity: {
      todo: string;
    };
  }
}

export const NoteActivity: ActivityComponentType<"NoteActivity"> = () => {
  const loaderData = useLoaderData<typeof noteActivityLoader>();

  return (
    <AppScreen appBar={{ title: "Hyprnote - Note" }}>
      <div>
        <h1>NoteActivity</h1>
        <p>{loaderData.todo}</p>
      </div>
    </AppScreen>
  );
};

import { defineConfig } from "@stackflow/config";

import { homeActivityLoader } from "./activities/home.loader";
import { noteActivityLoader } from "./activities/note.loader";

export const config = defineConfig({
  transitionDuration: 250,
  activities: [
    {
      name: "HomeActivity",
      loader: homeActivityLoader,
    },
    {
      name: "NoteActivity",
      loader: noteActivityLoader,
    },
  ],
  initialActivity: () => "HomeActivity",
});

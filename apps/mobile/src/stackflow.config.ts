import { defineConfig } from "@stackflow/config";
import { homeActivityLoader } from "./views/home";
import { noteActivityLoader } from "./views/note";
import { settingsActivityLoader } from "./views/settings";

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
    {
      name: "LoginActivity",
    },
    {
      name: "SettingsActivity",
      loader: settingsActivityLoader,
    },
  ],
  initialActivity: () => "HomeActivity",
});

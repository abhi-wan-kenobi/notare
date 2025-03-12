import { defineConfig } from "@stackflow/config";
import { homeLoader } from "./views/home";
import { noteLoader } from "./views/note";
import { recordingsLoader } from "./views/recordings";
import { settingsLoader } from "./views/settings";

export const config = defineConfig({
  transitionDuration: 250,
  activities: [
    {
      name: "HomeView",
      loader: homeLoader,
    },
    {
      name: "NoteView",
      loader: noteLoader,
    },
    {
      name: "LoginView",
    },
    {
      name: "SettingsView",
      loader: settingsLoader,
    },
    {
      name: "RecordingsView",
      loader: recordingsLoader,
    },
  ],
  initialActivity: () => "HomeView",
});

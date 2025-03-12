import { defineConfig } from "@stackflow/config";
import { homeActivityLoader } from "./views/home";
import { noteActivityLoader } from "./views/note";
import { profileActivityLoader } from "./views/profile";
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
    {
      name: "ProfileActivity",
      loader: profileActivityLoader,
    },
  ],
  initialActivity: () => "HomeActivity",
});

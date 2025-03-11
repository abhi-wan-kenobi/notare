import { basicUIPlugin } from "@stackflow/plugin-basic-ui";
import { basicRendererPlugin } from "@stackflow/plugin-renderer-basic";
import { stackflow } from "@stackflow/react/future";
import { config } from "./stackflow.config";
import { HomeActivity } from "./views/home";
import { LoginActivity } from "./views/login";
import { NoteActivity } from "./views/note";
import { SettingsActivity } from "./views/settings";
import { ProfileActivity } from "./views/profile";

export const { Stack } = stackflow({
  config,
  components: {
    HomeActivity,
    NoteActivity,
    LoginActivity,
    SettingsActivity,
    ProfileActivity
  },
  plugins: [basicRendererPlugin(), basicUIPlugin({ theme: "cupertino" })],
});

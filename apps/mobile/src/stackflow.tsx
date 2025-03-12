import { basicUIPlugin } from "@stackflow/plugin-basic-ui";
import { basicRendererPlugin } from "@stackflow/plugin-renderer-basic";
import { stackflow } from "@stackflow/react/future";
import { config } from "./stackflow.config";
import { HomeView } from "./views/home";
import { LoginView } from "./views/login";
import { NoteView } from "./views/note";
import { RecordingsView } from "./views/recordings";
import { SettingsView } from "./views/settings";

export const { Stack } = stackflow({
  config,
  components: {
    HomeView,
    NoteView,
    LoginView,
    SettingsView,
    RecordingsView,
  },
  plugins: [basicRendererPlugin(), basicUIPlugin({ theme: "cupertino" })],
});

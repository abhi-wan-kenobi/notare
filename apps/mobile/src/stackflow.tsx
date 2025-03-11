import { basicUIPlugin } from "@stackflow/plugin-basic-ui";
import { basicRendererPlugin } from "@stackflow/plugin-renderer-basic";
import { stackflow } from "@stackflow/react/future";
import { HomeActivity } from "./views/home";
import { LoginActivity } from "./views/login";
import { OnboardingActivity } from "./views/onboarding";
import { NoteActivity } from "./views/note";
import { config } from "./stackflow.config";

export const { Stack } = stackflow({
  config,
  components: {
    HomeActivity,
    NoteActivity,
    LoginActivity,
    OnboardingActivity,
  },
  plugins: [basicRendererPlugin(), basicUIPlugin({ theme: "cupertino" })],
});

import { stackflow } from "@stackflow/react/future";
import { basicRendererPlugin } from "@stackflow/plugin-renderer-basic";
import { basicUIPlugin } from "@stackflow/plugin-basic-ui";

import { config } from "./stackflow.config";

import { HomeActivity } from "./activities/home";
import { NoteActivity } from "./activities/note";

export const { Stack } = stackflow({
  config,
  components: {
    HomeActivity: HomeActivity,
    NoteActivity: NoteActivity,
  },
  plugins: [basicRendererPlugin(), basicUIPlugin({ theme: "cupertino" })],
});

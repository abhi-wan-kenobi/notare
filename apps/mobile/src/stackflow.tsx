import { basicUIPlugin } from "@stackflow/plugin-basic-ui";
import { basicRendererPlugin } from "@stackflow/plugin-renderer-basic";
import { stackflow } from "@stackflow/react/future";

import { config } from "./stackflow.config";

import { HomeActivity } from "./activities/home";
import { SessionActivity } from "./activities/session";

export const { Stack } = stackflow({
  config,
  components: {
    HomeActivity,
    SessionActivity,
  },
  plugins: [basicRendererPlugin(), basicUIPlugin({ theme: "cupertino" })],
});

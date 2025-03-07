import { defineConfig } from "@stackflow/config";

import { homeActivityLoader } from "./activities/home";
import { sessionActivityLoader } from "./activities/session";

export const config = defineConfig({
  transitionDuration: 250,
  activities: [
    {
      name: "HomeActivity",
      loader: homeActivityLoader,
    },
    {
      name: "SessionActivity",
      loader: sessionActivityLoader,
    },
  ],
  initialActivity: () => "HomeActivity",
});

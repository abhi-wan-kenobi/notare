import { type Extension } from "@hypr/extension-utils";
import WorldClock2x2 from "./widgets/world/2x2";
import { init } from "./widgets/world/init";

const extension: Extension = {
  world: [
    {
      id: "world-clock-2x2",
      init,
      component: WorldClock2x2,
    },
  ],
};

export default extension;

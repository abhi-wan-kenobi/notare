import type { Extension } from "@hypr/extension-utils";
import ChromeDino2x1 from "./widgets/chrome/2x1";
import { init } from "./init";

const extension: Extension = {
  chromeDino: [
    {
      id: "dino-game-chrome-2x1",
      init,
      component: ChromeDino2x1,
    },
  ],
};

export default extension;

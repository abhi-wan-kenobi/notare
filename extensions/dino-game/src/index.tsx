import type { Extension } from "@hypr/extension-types";
import ChromeDino from "./widgets/chrome";

export default {
  id: "@hypr/extension-dino-game",
  widgetGroups: [ChromeDino],
  init: async () => {},
} satisfies Extension;

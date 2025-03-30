import type { Extension } from "@hypr/extension-types";

import Checkpoint from "./widgets/checkpoint";
import Default from "./widgets/default";

export default {
  id: "@hypr/extension-transcript",
  widgetGroups: [Default, Checkpoint],
  init: async () => {},
} satisfies Extension;

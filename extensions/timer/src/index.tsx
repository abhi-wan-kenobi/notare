import type { Extension } from "@hypr/extension-types";

import Analog from "./widgets/analog";
import Digital from "./widgets/digital";

export default {
  id: "@hypr/extension-timer",
  widgetGroups: [Analog, Digital],
  init: async () => {},
} satisfies Extension;

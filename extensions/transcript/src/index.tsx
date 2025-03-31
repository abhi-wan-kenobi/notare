import type { Extension } from "@hypr/extension-types";

import Default from "./widgets/default";

export default {
  id: "@hypr/extension-transcript",
  widgetGroups: [Default],
  init: async () => {},
} satisfies Extension;

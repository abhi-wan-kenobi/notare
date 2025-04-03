import { WidgetGroup } from "@hypr/extension-types";

import Widget2x1 from "./2x1";

export default {
  id: "twenty-default",
  items: [
    {
      type: "twoByOne",
      component: Widget2x1,
    },
  ],
} satisfies WidgetGroup;

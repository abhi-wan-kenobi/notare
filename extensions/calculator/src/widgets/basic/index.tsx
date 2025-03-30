import { WidgetGroup } from "@hypr/extension-types";
import Widget2x2 from "./2x2";

export default {
  id: "basic-calculator",
  items: [
    {
      type: "twoByTwo",
      component: Widget2x2,
    },
  ],
} satisfies WidgetGroup;

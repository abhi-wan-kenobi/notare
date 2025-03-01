import { type WidgetGroup } from "@hypr/extension-utils";
import Widget2x1 from "./2x1";

const widgetGroup: WidgetGroup = {
  id: "timer-digits",
  items: [
    {
      init: async () => {},
      type: "twoByOne",
      component: Widget2x1,
    },
  ],
};

export default widgetGroup;

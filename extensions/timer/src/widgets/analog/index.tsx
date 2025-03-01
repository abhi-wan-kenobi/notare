import { type WidgetGroup } from "@hypr/extension-utils";
import Widget2x2 from "./2x2";

const widgetGroup: WidgetGroup = {
  id: "timer-analog",
  items: [
    {
      init: async () => {},
      type: "twoByTwo",
      component: Widget2x2,
    },
  ],
};

export default widgetGroup;

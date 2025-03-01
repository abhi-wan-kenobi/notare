import { WidgetGroup } from "@hypr/extension-utils";
import Widget2x2 from "./2x2";

const widgetGroup: WidgetGroup = {
  id: "transcript-live",
  items: [
    {
      init: async () => {
        console.log("Transcript live widget initialized");
      },
      type: "twoByTwo",
      component: Widget2x2,
    },
  ],
};

export default widgetGroup;

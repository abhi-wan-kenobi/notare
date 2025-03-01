import { WidgetGroup } from "@hypr/extension-utils";
import Widget2x2 from "./2x2";
import WidgetFull from "./full";

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
    {
      init: async () => {
        console.log("Transcript live widget initialized");
      },
      type: "full",
      component: WidgetFull,
    },
  ],
};

export default widgetGroup;

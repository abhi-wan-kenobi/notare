import { WidgetGroup } from "@hypr/extension-utils";
import Widget2x1 from "./2x1";

const widgetGroup: WidgetGroup = {
  id: "dino-game-chrome",
  items: [
    {
      init: async () => {
        console.log("ChromeDino widget initialized");
      },
      type: "twoByOne",
      component: Widget2x1,
    },
  ],
};

export default widgetGroup;

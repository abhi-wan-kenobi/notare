import { type Extension } from "@hypr/extension-types";
import clockWorld from "./widgets/world";

export default {
  id: "@hypr/extension-clock",
  widgetGroups: [clockWorld],
  init: async () => {},
} satisfies Extension;

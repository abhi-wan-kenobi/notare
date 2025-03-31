import type { WidgetGroup } from "@hypr/extension-types";

import Widget2x2 from "./2x2";
import systemTemplate from "./system.jinja?raw";
import userTemplate from "./user.jinja?raw";

export const TEMPLATE_SYSTEM_KEY = "live-summary-bullet-system";
export const TEMPLATE_USER_KEY = "live-summary-bullet-user";

export const TEMPLATE_SYSTEM_VALUE = systemTemplate;
export const TEMPLATE_USER_VALUE = userTemplate;

export default {
  id: "bullet",
  items: [
    {
      type: "twoByTwo",
      component: Widget2x2,
    },
  ],
} satisfies WidgetGroup;

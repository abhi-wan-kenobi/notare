import type { Extension } from "@hypr/extension-types";
import { commands as templateCommands } from "@hypr/plugin-template";

import Bullet, {
  TEMPLATE_SYSTEM_KEY,
  TEMPLATE_SYSTEM_VALUE,
  TEMPLATE_USER_KEY,
  TEMPLATE_USER_VALUE,
} from "./widgets/bullet";

export default {
  id: "@hypr/extension-summary",
  widgetGroups: [Bullet],
  init: async () => {
    await Promise.all([
      templateCommands.registerTemplate(TEMPLATE_USER_KEY, TEMPLATE_USER_VALUE),
      templateCommands.registerTemplate(TEMPLATE_SYSTEM_KEY, TEMPLATE_SYSTEM_VALUE),
    ]);
  },
} satisfies Extension;

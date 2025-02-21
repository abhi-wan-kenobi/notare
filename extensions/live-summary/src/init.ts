import type { Extension } from "../../types";
import { commands as templateCommands } from "@hypr/plugin-template";

import systemTemplate from "./system.jinja?raw";
import userTemplate from "./user.jinja?raw";

export const TEMPLATE_LIVE_SUMMARY_SYSTEM = "live-summary-system";
export const TEMPLATE_LIVE_SUMMARY_USER = "live-summary-user";

const init: Extension["init"] = async () => {
  await Promise.all([
    templateCommands.registerTemplate(TEMPLATE_LIVE_SUMMARY_USER, userTemplate),
    templateCommands.registerTemplate(
      TEMPLATE_LIVE_SUMMARY_SYSTEM,
      systemTemplate,
    ),
  ]);
};

export default init;

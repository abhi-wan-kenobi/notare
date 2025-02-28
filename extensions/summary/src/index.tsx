import type { Extension } from "@hypr/extension-utils";
import { init } from "./widgets/live-short/init";
import LiveShortSummary2x2 from "./widgets/live-short/2x2";

const extension: Extension = {
  live: [
    {
      id: "summary-live-2x2",
      init,
      component: LiveShortSummary2x2,
    },
  ],
};

export default extension;

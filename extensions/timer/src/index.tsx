import type { Extension } from "@hypr/extension-utils";

import DiscTimer2x2 from "./widgets/disc/2x2";
import DigitsTimer2x1 from "./widgets/digits/2x1";
import { init as discInit } from "./widgets/disc/init";
import { init as digitsInit } from "./widgets/digits/init";

const extension: Extension = {
  analog: [
    {
      id: "timer-disc-2x2",
      init: discInit,
      component: DiscTimer2x2,
    },
  ],
  digital: [
    {
      id: "timer-digits-2x1",
      init: digitsInit,
      component: DigitsTimer2x1,
    },
  ],
};

export default extension;

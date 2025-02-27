import type { Extension } from "@hypr/extension-utils";

import BasicCalculator2x2 from "./widgets/basic/2x2";
import { init } from "./widgets/basic/init";

const extension: Extension = {
  basic: [
    {
      id: "calculator-basic-2x2",
      init,
      component: BasicCalculator2x2,
    },
  ],
};

export default extension;

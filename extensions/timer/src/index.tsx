import type { Extension } from "@hypr/extension-utils";

import Disc2x2 from "./widgets/disc/2x2";
import init from "./widgets/disc/init";

const extension: Extension = {
  disc: [
    {
      init,
      widget: Disc2x2,
    },
  ],
};

export default extension;

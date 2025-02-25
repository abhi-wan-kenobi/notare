import type { Extension } from "@hypr/extension-utils";
import TwoByOne from "./two-by-one";

import init from "./init";

const extension: Extension = {
  init,
  twoByOne: TwoByOne,
};

export default extension;

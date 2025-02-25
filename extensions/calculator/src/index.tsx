import type { Extension } from "@hypr/extension-utils";

import TwoByTwo from "./two-by-two";
import init from "./init";

const extension: Extension = {
  init,
  twoByTwo: TwoByTwo,
};

export default extension;

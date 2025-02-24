import type { Extension } from "@hypr/extension-utils";

import widget from "./main";
import init from "./init";

const extension: Extension = {
  init,
  twoByTwo: widget,
};

export default extension;

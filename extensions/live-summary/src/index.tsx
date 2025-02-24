import type { Extension } from "@hypr/extension-utils";

import widget from "./widget";
import init from "./init";

const extension: Extension = {
  init,
  twoByTwo: widget,
};

export default extension;

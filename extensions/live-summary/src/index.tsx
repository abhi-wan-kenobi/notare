import type { Extension } from "@hypr/extension-utils";

import modal from "./modal";
import init from "./init";

const extension: Extension = {
  init,
  modal,
};

export default extension;

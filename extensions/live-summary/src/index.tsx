import type { Extension } from "../../types";

import modal from "./modal";
import init from "./init";

const extension: Extension = {
  init,
  modal,
};

export default extension;

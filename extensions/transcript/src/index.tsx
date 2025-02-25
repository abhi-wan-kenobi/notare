import type { Extension } from "@hypr/extension-utils";

import TwoByTwo from "./two-by-two";
import FullSizeModal from "./full-size-modal";
import init from "./init";

const extension: Extension = {
  init,
  twoByTwo: TwoByTwo,
  full: FullSizeModal,
};

export default extension;

import type { Extension } from "@hypr/extension-utils";

import Live from "./widgets/live";
import Default from "./widgets/default";

const extension: Extension = {
  live: Live,
  default: Default,
};

export default extension;

import type { Extension } from "@hypr/extension-utils";
import Checkpoint from "./widgets/checkpoint";
import Default from "./widgets/default";

const extension: Extension = {
  default: Default,
  checkpoint: Checkpoint,
};

export default extension;

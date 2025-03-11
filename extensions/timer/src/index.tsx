import type { Extension } from "@hypr/extension-utils";
import Analog from "./widgets/analog";
import Digital from "./widgets/digital";

const extension: Extension = {
  analog: Analog,
  digital: Digital,
};

export default extension;

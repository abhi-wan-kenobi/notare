import type { Extension } from "@hypr/extension-utils";

import LiveTranslation from "./widgets/live";

const extension: Extension = {
  liveTranslate: LiveTranslation,
};

export default extension;

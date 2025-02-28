import type { Extension } from "@hypr/extension-utils";

import LiveTranslataion2x2 from "./widgets/live/2x2";
import LiveTranslationFull from "./widgets/live/full";
import { init } from "./widgets/live/init";

const extension: Extension = {
  liveTranslate: [
    {
      id: "translation-live-2x2",
      init: init,
      component: LiveTranslataion2x2,
    },
    {
      id: "translation-live-full",
      init: init,
      component: LiveTranslationFull,
    },
  ],
};

export default extension;

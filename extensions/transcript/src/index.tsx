import type { Extension } from "@hypr/extension-utils";

import TwoByTwo from "./widgets/live/2x2";
import FullSizeModal from "./widgets/live/full";
import init from "./widgets/live/init";

const extension: Extension = {
  liveTranscript: [
    {
      id: "transcript-live-2x2",
      init: init,
      component: TwoByTwo,
    },
    {
      id: "transcript-live-full",
      init: init,
      component: FullSizeModal,
    },
  ],
};

export default extension;

import type { Extension } from "@hypr/extension-utils";

import LiveTranscript2x2 from "./widgets/default/2x2";
import LiveTranscriptFull from "./widgets/default/full";
import { init } from "./widgets/default/init";

const extension: Extension = {
  liveTranscript: [
    {
      id: "transcript-live-2x2",
      init: init,
      component: LiveTranscript2x2,
    },
    {
      id: "transcript-live-full",
      init: init,
      component: LiveTranscriptFull,
    },
  ],
};

export default extension;

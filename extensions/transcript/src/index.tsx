import type { Extension } from "@hypr/extension-utils";

import LiveTranscript2x2 from "./widgets/live/2x2";
import LiveTranscriptFull from "./widgets/live/full";
import { init } from "./widgets/live/init";

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

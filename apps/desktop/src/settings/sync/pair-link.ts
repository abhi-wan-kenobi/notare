import { isTauri } from "@tauri-apps/api/core";
import { useEffect } from "react";

import {
  commands as deeplink2Commands,
  events as deeplink2Events,
} from "@hypr/plugin-deeplink2";

import { usePendingPairLink } from "./pending-pair-link";

import { useTabs } from "~/store/zustand/tabs";

/**
 * Opens Settings → Devices with the link queued. Must be mounted in the main
 * window only (the tab strip lives there).
 *
 * The native side parks the link and only emits an event as a wake-up, so
 * the same drain serves a cold start (link delivered before this listener
 * existed) and a warm one, and a link is consumed exactly once.
 */
export function usePairLinkHandler() {
  const openNew = useTabs((state) => state.openNew);

  useEffect(() => {
    if (!isTauri()) {
      return;
    }

    const drain = async () => {
      const pending = await deeplink2Commands.takePendingPair();
      if (!pending) {
        return;
      }
      usePendingPairLink.getState().set(pending.link);
      openNew({
        type: "settings",
        state: { tab: "integrations", section: "sync" },
      });
    };

    const unlisten = deeplink2Events.deepLinkEvent.listen(({ payload }) => {
      if (payload.to === "/pair") {
        void drain();
      }
    });
    void unlisten.then(() => drain());

    return () => {
      void unlisten.then((fn) => fn());
    };
  }, [openNew]);
}

import { ListenButton } from "./listen";

import {
  useCurrentNoteTab,
  useHasTranscript,
} from "~/session/components/shared";
import { ChatCTA } from "~/shared/chat-cta";
import type { Tab } from "~/store/zustand/tabs/schema";

export function FloatingActionButton({
  tab,
}: {
  tab: Extract<Tab, { type: "sessions" }>;
}) {
  const shouldShowListen = useShouldShowListeningFab(tab);

  return (
    <div className="absolute bottom-4 left-1/2 z-20 -translate-x-1/2">
      {shouldShowListen ? <ListenButton tab={tab} /> : <ChatCTA />}
    </div>
  );
}

export function useShouldShowListeningFab(
  tab: Extract<Tab, { type: "sessions" }>,
) {
  const currentTab = useCurrentNoteTab(tab);
  const hasTranscript = useHasTranscript(tab.id);

  return currentTab.type === "raw" && !hasTranscript;
}

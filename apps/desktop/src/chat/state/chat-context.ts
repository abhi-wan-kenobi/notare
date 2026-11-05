import { create } from "zustand";

import { id } from "~/shared/utils";

interface ChatContextState {
  groupId: string | undefined;
  sessionId: string;
  /**
   * Text the inspector's dock/ask-anywhere entry point wants to send as the
   * first message once the chat session mounts and the model is ready.
   * `ChatContent` consumes and clears this.
   */
  pendingPrompt: string | null;
}

interface ChatContextActions {
  setGroupId: (groupId: string | undefined) => void;
  startNewChat: () => void;
  selectChat: (groupId: string) => void;
  setPendingPrompt: (prompt: string | null) => void;
}

export const useChatContext = create<ChatContextState & ChatContextActions>(
  (set) => ({
    groupId: undefined,
    sessionId: id(),
    pendingPrompt: null,
    setGroupId: (groupId) => set({ groupId }),
    startNewChat: () => set({ groupId: undefined, sessionId: id() }),
    selectChat: (groupId) => set({ groupId, sessionId: groupId }),
    setPendingPrompt: (pendingPrompt) => set({ pendingPrompt }),
  }),
);

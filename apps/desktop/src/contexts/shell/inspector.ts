import { useCallback, useMemo } from "react";

import { useChatContext } from "~/chat/state/chat-context";
import type { useChatMode } from "~/chat/state/use-chat-mode";

/**
 * The 360px docked drawer contract Wave 2 consumes: `useShell().inspector`.
 * Wraps the existing chat-mode slice (now a plain Open/Closed state) so the
 * ⌘J hotkey and navigation-close behavior already wired into `useChatMode`
 * keep working unchanged.
 */
export function useInspector(chat: ReturnType<typeof useChatMode>) {
  const startNewChat = chat.startNewChat;
  const sendEvent = chat.sendEvent;
  const setPendingPrompt = useChatContext((state) => state.setPendingPrompt);

  const toggle = useCallback(() => {
    sendEvent({ type: "TOGGLE" });
  }, [sendEvent]);

  const close = useCallback(() => {
    sendEvent({ type: "CLOSE" });
  }, [sendEvent]);

  const openWith = useCallback(
    (prompt?: string) => {
      if (prompt) {
        startNewChat();
        setPendingPrompt(prompt);
      }
      sendEvent({ type: "OPEN" });
    },
    [sendEvent, setPendingPrompt, startNewChat],
  );

  return useMemo(
    () => ({
      open: chat.mode === "Open",
      toggle,
      openWith,
      close,
    }),
    [chat.mode, toggle, openWith, close],
  );
}

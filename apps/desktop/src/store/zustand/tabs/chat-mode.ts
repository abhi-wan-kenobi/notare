import type { StoreApi } from "zustand";

/**
 * The inspector (`contexts/shell/inspector.ts`) is a single docked drawer —
 * there is no more floating-vs-right-panel distinction. This slice only
 * tracks whether it is open.
 */
export type ChatMode = "Open" | "Closed";

export type ChatEvent =
  | { type: "OPEN" }
  | { type: "CLOSE" }
  | { type: "TOGGLE" };

export type ChatModeState = {
  chatMode: ChatMode;
};

export type ChatModeActions = {
  transitionChatMode: (event: ChatEvent) => void;
};

const computeNextChatMode = (state: ChatMode, event: ChatEvent): ChatMode => {
  switch (event.type) {
    case "OPEN":
      return "Open";
    case "CLOSE":
      return "Closed";
    case "TOGGLE":
      return state === "Open" ? "Closed" : "Open";
    default:
      return state;
  }
};

export const createChatModeSlice = <T extends ChatModeState>(
  set: StoreApi<T>["setState"],
  get: StoreApi<T>["getState"],
): ChatModeState & ChatModeActions => ({
  chatMode: "Closed",
  transitionChatMode: (event) => {
    const currentMode = get().chatMode;
    const nextMode = computeNextChatMode(currentMode, event);
    if (nextMode === currentMode) return;

    set({
      chatMode: nextMode,
    } as Partial<T>);
  },
});

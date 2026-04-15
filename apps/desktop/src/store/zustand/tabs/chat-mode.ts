import type { StoreApi } from "zustand";

export type ChatMode = "RightPanelOpen" | "FloatingClosed" | "FloatingOpen";

export type ChatEvent =
  | { type: "OPEN" }
  | { type: "OPEN_FLOATING" }
  | { type: "OPEN_RIGHT_PANEL" }
  | { type: "CLOSE" }
  | { type: "SHIFT" }
  | { type: "TOGGLE" };

export type ChatModeState = {
  chatMode: ChatMode;
  lastOpenChatMode: "FloatingOpen" | "RightPanelOpen";
};

export type ChatModeActions = {
  transitionChatMode: (event: ChatEvent) => void;
};

const computeNextChatMode = (
  state: ChatMode,
  event: ChatEvent,
  lastOpenMode: "FloatingOpen" | "RightPanelOpen",
): ChatMode => {
  switch (state) {
    case "RightPanelOpen":
      if (event.type === "OPEN_FLOATING") {
        return "FloatingOpen";
      }
      if (event.type === "CLOSE" || event.type === "TOGGLE") {
        return "FloatingClosed";
      }
      if (event.type === "SHIFT") {
        return "FloatingOpen";
      }
      return state;
    case "FloatingClosed":
      if (event.type === "OPEN" || event.type === "TOGGLE") {
        return lastOpenMode;
      }
      if (event.type === "OPEN_FLOATING") {
        return "FloatingOpen";
      }
      if (event.type === "OPEN_RIGHT_PANEL") {
        return "RightPanelOpen";
      }
      return state;
    case "FloatingOpen":
      if (event.type === "OPEN_RIGHT_PANEL") {
        return "RightPanelOpen";
      }
      if (event.type === "CLOSE" || event.type === "TOGGLE") {
        return "FloatingClosed";
      }
      if (event.type === "SHIFT") {
        return "RightPanelOpen";
      }
      return state;
    default:
      return state;
  }
};

export const createChatModeSlice = <T extends ChatModeState>(
  set: StoreApi<T>["setState"],
  get: StoreApi<T>["getState"],
): ChatModeState & ChatModeActions => ({
  chatMode: "FloatingClosed",
  lastOpenChatMode: "FloatingOpen",
  transitionChatMode: (event) => {
    const currentMode = get().chatMode;
    const lastOpenMode = get().lastOpenChatMode;
    const nextMode = computeNextChatMode(currentMode, event, lastOpenMode);
    if (nextMode === currentMode) return;

    set({
      chatMode: nextMode,
      ...(currentMode === "FloatingOpen" || currentMode === "RightPanelOpen"
        ? { lastOpenChatMode: currentMode }
        : {}),
    } as Partial<T>);
  },
});

import { create } from "zustand";

import { requestSeek } from "~/audio-player/pending-seek";
import { useTabs } from "~/store/zustand/tabs";

export interface SessionStoreState {
  activeSessionId: string | null;
  setActiveSessionId: (sessionId: string | null) => void;
  seekTo: (ms: number, sessionId?: string) => void;
}

export const useSessionStore = create<SessionStoreState>((set, get) => ({
  activeSessionId: null,
  setActiveSessionId: (activeSessionId) => set({ activeSessionId }),
  seekTo: (ms: number, sessionId?: string) => {
    const targetSessionId =
      sessionId ??
      get().activeSessionId ??
      (() => {
        const currentTab = useTabs.getState().currentTab;
        return currentTab?.type === "sessions" ? currentTab.id : null;
      })();

    if (!targetSessionId || !Number.isFinite(ms) || ms < 0) return;

    requestSeek(targetSessionId, ms);

    if (typeof window !== "undefined") {
      window.dispatchEvent(
        new CustomEvent("notare:seek", {
          detail: { sessionId: targetSessionId, ms },
        }),
      );
    }
  },
}));

export const sessionStore = {
  seekTo: (ms: number, sessionId?: string) =>
    useSessionStore.getState().seekTo(ms, sessionId),
};

export function seekTo(ms: number, sessionId?: string): void;
export function seekTo(sessionId: string, ms: number): void;
export function seekTo(first: string | number, second?: string | number): void {
  if (typeof first === "number") {
    useSessionStore
      .getState()
      .seekTo(first, typeof second === "string" ? second : undefined);
  } else if (typeof first === "string" && typeof second === "number") {
    useSessionStore.getState().seekTo(second, first);
  }
}

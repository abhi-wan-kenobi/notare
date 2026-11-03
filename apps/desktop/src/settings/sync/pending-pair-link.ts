import { create } from "zustand";

/**
 * A `notare://pair#…` link that arrived from the OS and has not been handed
 * to the "Join with a link" flow yet. It survives the Devices page being
 * closed, still loading, or sync being switched off, and is never persisted
 * or logged: the fragment carries the pairing secret.
 */
export const usePendingPairLink = create<{
  link: string | null;
  set: (link: string) => void;
  clear: () => void;
}>((set) => ({
  link: null,
  set: (link) => set({ link }),
  clear: () => set({ link: null }),
}));

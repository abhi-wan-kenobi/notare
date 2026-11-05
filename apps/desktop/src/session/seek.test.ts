import { beforeEach, describe, expect, it, vi } from "vitest";

import { seekTo, useSessionStore } from "./seek";

import {
  __resetPendingSeek,
  hasPendingSeek,
} from "~/audio-player/pending-seek";

describe("session store seekTo", () => {
  beforeEach(() => {
    __resetPendingSeek();
    useSessionStore.getState().setActiveSessionId(null);
  });

  it("requests audio seek and dispatches notare:seek event", () => {
    useSessionStore.getState().setActiveSessionId("session-abc");
    const listener = vi.fn();
    window.addEventListener("notare:seek", listener);

    seekTo(42000);

    expect(hasPendingSeek("session-abc")).toBe(true);
    expect(listener).toHaveBeenCalled();
    const event = listener.mock.calls[0][0] as CustomEvent<{
      sessionId: string;
      ms: number;
    }>;
    expect(event.detail).toEqual({ sessionId: "session-abc", ms: 42000 });

    window.removeEventListener("notare:seek", listener);
  });

  it("accepts sessionId as first or second argument", () => {
    seekTo("session-explicit", 15000);
    expect(hasPendingSeek("session-explicit")).toBe(true);
  });

  it("ignores negative or non-finite timestamps", () => {
    useSessionStore.getState().setActiveSessionId("session-abc");
    seekTo(-1);
    seekTo(Number.NaN);
    expect(hasPendingSeek("session-abc")).toBe(false);
  });
});

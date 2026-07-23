import { afterEach, describe, expect, it, vi } from "vitest";

import {
  __resetPendingSeek,
  consumeSeek,
  hasPendingSeek,
  requestSeek,
  subscribePendingSeek,
} from "./pending-seek";

afterEach(() => __resetPendingSeek());

describe("pending-seek channel", () => {
  it("delivers a request to the matching session exactly once", () => {
    requestSeek("s1", 4200);
    expect(hasPendingSeek("s1")).toBe(true);
    expect(consumeSeek("s1")).toBe(4200);
    // Single delivery — second consume is empty.
    expect(consumeSeek("s1")).toBeNull();
    expect(hasPendingSeek("s1")).toBe(false);
  });

  it("does not deliver to a different session", () => {
    requestSeek("s1", 1000);
    expect(consumeSeek("s2")).toBeNull();
    // still pending for s1
    expect(consumeSeek("s1")).toBe(1000);
  });

  it("a newer request overwrites an unconsumed one", () => {
    requestSeek("s1", 1000);
    requestSeek("s1", 9000);
    expect(consumeSeek("s1")).toBe(9000);
  });

  it("negative ms clears any pending request", () => {
    requestSeek("s1", 1000);
    requestSeek("s1", -1);
    expect(hasPendingSeek("s1")).toBe(false);
    expect(consumeSeek("s1")).toBeNull();
  });

  it("notifies subscribers on request and on consume", () => {
    const listener = vi.fn();
    const unsub = subscribePendingSeek(listener);
    requestSeek("s1", 500);
    expect(listener).toHaveBeenCalledTimes(1);
    consumeSeek("s1");
    expect(listener).toHaveBeenCalledTimes(2);
    unsub();
    requestSeek("s1", 700);
    expect(listener).toHaveBeenCalledTimes(2); // unsubscribed
  });
});

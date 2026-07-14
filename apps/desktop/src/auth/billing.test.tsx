import { renderHook } from "@testing-library/react";
import type { ReactNode } from "react";
import { describe, expect, it } from "vitest";

import { BillingProvider, useBillingAccess } from "./billing";

// Notare has no hosted tier: billing is a static local context. These tests
// pin the invariants the rest of the app relies on — permanently free plan,
// no trial machinery, inert upgrade action.

function wrapper({ children }: { children: ReactNode }) {
  return <BillingProvider>{children}</BillingProvider>;
}

describe("BillingProvider", () => {
  it("is always ready with a free local plan", () => {
    const { result } = renderHook(() => useBillingAccess(), { wrapper });

    expect(result.current.isReady).toBe(true);
    expect(result.current.plan).toBe("free");
    expect(result.current.isPaid).toBe(false);
    expect(result.current.isPro).toBe(false);
    expect(result.current.isTrialing).toBe(false);
    expect(result.current.entitlements).toEqual([]);
  });

  it("never offers a trial", () => {
    const { result } = renderHook(() => useBillingAccess(), { wrapper });

    expect(result.current.canStartTrial).toEqual({
      data: false,
      isPending: false,
    });
    expect(result.current.trialEnd).toBeNull();
    expect(result.current.trialDaysRemaining).toBeNull();
  });

  it("upgradeToPro is a no-op that never throws", () => {
    const { result } = renderHook(() => useBillingAccess(), { wrapper });

    expect(() => result.current.upgradeToPro()).not.toThrow();
  });
});

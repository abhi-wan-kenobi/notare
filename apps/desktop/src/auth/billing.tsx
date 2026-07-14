import { createContext, type ReactNode, useContext, useMemo } from "react";

import { type BillingInfo, deriveBillingInfo } from "@hypr/supabase";

// Notare has no hosted tier: billing is a static local context. Everything
// local is free; the (unreachable) upstream cloud providers stay locked
// because the plan is permanently "free" and trials can never start. This
// keeps the ~dozen useBillingAccess() consumers working without a network
// request or upsell dialog anywhere.

type BillingContextValue = BillingInfo & {
  isReady: boolean;
  canStartTrial: { data: boolean; isPending: boolean };
  upgradeToPro: () => void;
};

export type BillingAccess = BillingContextValue;

const BillingContext = createContext<BillingContextValue | null>(null);

export function BillingProvider({ children }: { children: ReactNode }) {
  const value = useMemo<BillingContextValue>(
    () => ({
      ...deriveBillingInfo(null),
      isReady: true,
      canStartTrial: { data: false, isPending: false },
      upgradeToPro: () => {},
    }),
    [],
  );

  return (
    <BillingContext.Provider value={value}>{children}</BillingContext.Provider>
  );
}

export function useBillingAccess() {
  const context = useContext(BillingContext);

  if (!context) {
    throw new Error("useBillingAccess must be used within BillingProvider");
  }

  return context;
}

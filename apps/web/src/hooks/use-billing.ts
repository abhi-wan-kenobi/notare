import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useCallback } from "react";

import {
  type BillingInfo,
  deriveBillingInfo,
  type SupabaseJwtPayload,
} from "@hypr/supabase";

import { getAccessToken } from "@/functions/access-token";
import { getSupabaseBrowserClient } from "@/functions/supabase";

function decodeJwtPayload(token: string): SupabaseJwtPayload {
  return JSON.parse(atob(token.split(".")[1]));
}

const DEFAULT_BILLING = deriveBillingInfo(null);

export function useBilling() {
  const queryClient = useQueryClient();

  const jwtQuery = useQuery({
    queryKey: ["billing", "jwt"],
    queryFn: async () => {
      const supabase = getSupabaseBrowserClient();
      await supabase.auth.refreshSession();
      const token = await getAccessToken();
      return deriveBillingInfo(decodeJwtPayload(token));
    },
    retry: false,
  });

  const billing: BillingInfo = jwtQuery.data ?? DEFAULT_BILLING;
  const isReady = !jwtQuery.isPending;
  const isVerified = isReady;

  const refreshBilling = useCallback(async () => {
    const supabase = getSupabaseBrowserClient();
    await supabase.auth.refreshSession();
    await queryClient.invalidateQueries({ queryKey: ["billing"] });
  }, [queryClient]);

  return {
    ...billing,
    isReady,
    isVerified,
    refreshBilling,
  };
}

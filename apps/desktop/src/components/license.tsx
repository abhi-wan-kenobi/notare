import { useEffect } from "react";

import { useLicense } from "@/hooks/use-license";

export function LicenseRefreshProvider({ children }: { children: React.ReactNode }) {
  const { shouldRefresh, refreshLicense, getLicense } = useLicense();

  useEffect(() => {
    if (getLicense.isLoading) {
      return;
    }

    refreshLicense.mutate();

    const checkAndRefresh = () => {
      if (shouldRefresh() && !refreshLicense.isPending) {
      }
    };

    checkAndRefresh();
    const interval = setInterval(checkAndRefresh, 1000 * 60 * 10); // 10min

    return () => clearInterval(interval);
  }, [shouldRefresh, refreshLicense, getLicense.isLoading, refreshLicense.isPending]);

  return <>{children}</>;
}

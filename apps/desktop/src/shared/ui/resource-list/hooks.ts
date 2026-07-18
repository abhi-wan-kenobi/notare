import { useQuery } from "@tanstack/react-query";

// The upstream "anarlog.so" template suggestion endpoint is gone in Notare.
// Keep the hook shape (consumers destructure { data, isLoading }) but never
// hit the old vendor — return an empty list locally.
export function useWebResources<T>(endpoint: string) {
  return useQuery({
    queryKey: ["settings", endpoint, "suggestions"],
    initialData: [],
    queryFn: async () => [] as T[],
  });
}

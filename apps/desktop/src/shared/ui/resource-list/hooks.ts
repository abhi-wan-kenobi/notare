import { useQuery } from "@tanstack/react-query";

// Built-in templates are bundled locally in Notare gallery.
import { BUILT_IN_TEMPLATES } from "~/templates/gallery-data";

export function useWebResources<T>(endpoint: string) {
  const initialData =
    endpoint === "templates"
      ? (BUILT_IN_TEMPLATES as unknown as T[])
      : ([] as T[]);

  return useQuery({
    queryKey: ["settings", endpoint, "suggestions"],
    initialData,
    queryFn: async () => initialData,
  });
}

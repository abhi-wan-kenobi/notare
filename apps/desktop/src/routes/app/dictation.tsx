import { createFileRoute } from "@tanstack/react-router";

import { DictationOrbWindow } from "~/dictation/window";

export const Route = createFileRoute("/app/dictation")({
  validateSearch: (search: Record<string, unknown>) => ({
    // Set by the Rust side when the OS window was created without
    // transparency (Windows fallback) — render the solid variant.
    solid: search.solid === "1" || search.solid === 1 || search.solid === true,
  }),
  component: DictationRoute,
});

function DictationRoute() {
  const { solid } = Route.useSearch();

  return <DictationOrbWindow solid={solid} />;
}

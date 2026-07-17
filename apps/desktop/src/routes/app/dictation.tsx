import { createFileRoute } from "@tanstack/react-router";

import { DictationCaptionWindow } from "~/dictation/caption";
import { DictationOrbWindow } from "~/dictation/window";

export const Route = createFileRoute("/app/dictation")({
  validateSearch: (search: Record<string, unknown>) => ({
    // Set by the Rust side when the OS window was created without
    // transparency (Windows fallback) — render the solid variant.
    solid: search.solid === "1" || search.solid === 1 || search.solid === true,
    // Set by the Rust side for the live-caption window that floats above
    // the orb (same route, second webview - avoids a second route file).
    caption:
      search.caption === "1" || search.caption === 1 || search.caption === true,
  }),
  component: DictationRoute,
});

function DictationRoute() {
  const { solid, caption } = Route.useSearch();

  return caption ? (
    <DictationCaptionWindow solid={solid} />
  ) : (
    <DictationOrbWindow solid={solid} />
  );
}

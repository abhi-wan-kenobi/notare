import { createFileRoute } from "@tanstack/react-router";
import { useEffect, useState } from "react";

import type { FloatingBarState } from "@hypr/plugin-windows";

import { FloatingBarContent, FloatingBarWindow } from "~/meeting-float/window";

export const Route = createFileRoute("/app/floating")({
  validateSearch: (search: Record<string, unknown>) => ({
    // Set by the Rust side when the OS window was created without
    // transparency (Windows fallback) — render the solid variant.
    solid: search.solid === "1" || search.solid === 1 || search.solid === true,
    // Dev-only preview harness (no Tauri events needed in a plain browser).
    demo:
      import.meta.env.DEV &&
      (search.demo === "1" || search.demo === 1 || search.demo === true),
  }),
  component: FloatingRoute,
});

function FloatingRoute() {
  const { solid, demo } = Route.useSearch();

  if (demo) {
    return <FloatingBarDemo solid={solid} />;
  }

  return <FloatingBarWindow solid={solid} />;
}

/** Animated canned state so the orb can be reviewed outside a recording. */
function FloatingBarDemo({ solid }: { solid: boolean }) {
  const [amplitude, setAmplitude] = useState(0.4);

  useEffect(() => {
    document.documentElement.classList.add("dark");
    const interval = setInterval(() => {
      setAmplitude(0.15 + Math.random() * 0.8);
    }, 160);
    return () => clearInterval(interval);
  }, []);

  const state: FloatingBarState = {
    amplitude,
    title: "Weekly sync",
    status: "recording",
    colorScheme: "dark",
    opacity: 0.85,
    liveCaptionOpacity: 0.3,
    liveCaptionWidth: 440,
    liveCaptionLineCount: 2,
    liveCaptionPosition: "topCenter",
    liveCaptionMinimized: false,
    liveCaptionToggleVisible: true,
    transcriptBubbles: [
      {
        id: "demo-1",
        speakerLabel: "Speaker 1",
        text: "Let's walk through the launch checklist first.",
        isSelf: false,
        isFinal: true,
        startMs: 0,
        endMs: 2400,
        overlapsPrevious: false,
        overlapsNext: false,
      },
      {
        id: "demo-2",
        speakerLabel: "You",
        text: "Sounds good, I'll share the model integrity notes after.",
        isSelf: true,
        isFinal: false,
        startMs: 2500,
        endMs: 4100,
        overlapsPrevious: false,
        overlapsNext: false,
      },
    ],
  };

  return (
    <div style={{ width: 440, height: 130 }}>
      <FloatingBarContent state={state} solid={solid} />
    </div>
  );
}

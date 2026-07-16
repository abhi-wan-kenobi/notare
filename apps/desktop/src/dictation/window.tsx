import { useLingui } from "@lingui/react/macro";
import { useEffect, useState } from "react";

import {
  type DictationStateEvent,
  events as dictationEvents,
} from "@hypr/plugin-dictation";
import { cn } from "@hypr/utils";

import { DictationOrb } from "./orb";

const IDLE_STATE: DictationStateEvent = { phase: "idle", amplitude: 0 };

/**
 * Content of the persistent dictation-orb webview window (Windows/Linux; the
 * Rust side of the dictation plugin creates a tiny always-on-top,
 * non-focusable window pointing at `/app/dictation`).
 *
 * The Rust dictation session broadcasts `DictationStateEvent`s; clicking the
 * orb emits `DictationOrbClicked`, which the main-window host turns into a
 * session toggle. The window is non-focusable so the click never steals
 * keyboard focus from the app receiving the dictated text.
 *
 * `solid` mirrors the floating bar's fallback: when the OS refused a
 * transparent window, the Rust side loads `/app/dictation?solid=1` and this
 * renders on an opaque rounded surface instead of a bare orb.
 */
export function DictationOrbWindow({ solid = false }: { solid?: boolean }) {
  const { t } = useLingui();
  const state = useDictationState();

  useEffect(() => {
    // The orb is designed on the graphite (dark) token set.
    document.documentElement.classList.add("dark");

    if (solid) {
      return;
    }

    document.documentElement.style.background = "transparent";
    document.body.style.background = "transparent";
  }, [solid]);

  const dictating = state.phase === "listening" || state.phase === "processing";
  const label = dictating ? t`Stop dictation` : t`Start dictation`;

  const handleClick = () => {
    void dictationEvents.dictationOrbClicked.emit({});
  };

  return (
    <div
      data-testid={solid ? "dictation-window-solid" : "dictation-window-glass"}
      className={cn([
        "flex h-screen w-screen items-center justify-center overflow-hidden",
        solid && "bg-background border-border rounded-xl border",
      ])}
    >
      <button
        type="button"
        aria-label={label}
        title={label}
        aria-pressed={dictating}
        onClick={handleClick}
        className={cn([
          "flex cursor-pointer items-center justify-center rounded-full",
          "focus-visible:ring-ring focus-visible:ring-2 focus-visible:outline-hidden",
        ])}
      >
        <DictationOrb
          phase={state.phase}
          amplitude={state.amplitude}
          size={40}
        />
      </button>
    </div>
  );
}

function useDictationState() {
  const [state, setState] = useState<DictationStateEvent>(IDLE_STATE);

  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | null = null;

    void dictationEvents.dictationStateEvent
      .listen((event) => {
        if (!cancelled) {
          setState(event.payload);
        }
      })
      .then((next) => {
        if (cancelled) {
          next();
          return;
        }

        unlisten = next;
      });

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  return state;
}

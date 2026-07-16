import { useLingui } from "@lingui/react/macro";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { useEffect, useRef, useState } from "react";

import {
  type DictationStateEvent,
  events as dictationEvents,
} from "@hypr/plugin-dictation";
import { cn } from "@hypr/utils";

import { DictationOrb } from "./orb";

const IDLE_STATE: DictationStateEvent = {
  phase: "idle",
  amplitude: 0,
  mode: "type",
};

/**
 * Pointer travel (px) past which a press on the orb becomes a window drag
 * instead of a click. Mirrors the usual drag-threshold pattern: we cannot use
 * `data-tauri-drag-region` here because it eats the mousedown and the click
 * would never toggle dictation.
 */
const DRAG_THRESHOLD_PX = 4;

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
  const pointerStart = useRef<{ x: number; y: number } | null>(null);
  const draggedRef = useRef(false);

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
  const batchMode = state.mode === "batch-paste";
  const label = dictating
    ? batchMode
      ? t`Stop dictation and paste the transcript`
      : t`Stop dictation`
    : t`Start dictation`;

  const handlePointerDown = (event: React.PointerEvent) => {
    if (event.button !== 0) {
      return;
    }
    draggedRef.current = false;
    pointerStart.current = { x: event.screenX, y: event.screenY };
  };

  const handlePointerMove = (event: React.PointerEvent) => {
    const start = pointerStart.current;
    if (!start || draggedRef.current) {
      return;
    }
    const dx = event.screenX - start.x;
    const dy = event.screenY - start.y;
    if (Math.hypot(dx, dy) < DRAG_THRESHOLD_PX) {
      return;
    }
    // Past the threshold: hand the gesture to the OS window-move loop. The
    // click that would normally follow is suppressed via `draggedRef`.
    draggedRef.current = true;
    pointerStart.current = null;
    void getCurrentWebviewWindow().startDragging();
  };

  const handlePointerEnd = () => {
    pointerStart.current = null;
  };

  const handleClick = () => {
    if (draggedRef.current) {
      draggedRef.current = false;
      return;
    }
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
        data-dictation-mode={state.mode}
        onPointerDown={handlePointerDown}
        onPointerMove={handlePointerMove}
        onPointerUp={handlePointerEnd}
        onPointerCancel={handlePointerEnd}
        onClick={handleClick}
        className={cn([
          "relative flex cursor-pointer items-center justify-center rounded-full",
          "focus-visible:ring-ring focus-visible:ring-2 focus-visible:outline-hidden",
        ])}
      >
        <DictationOrb
          phase={state.phase}
          amplitude={state.amplitude}
          size={40}
        />
        {batchMode && dictating ? (
          // Subtle batch-mode hint: a small cobalt dot marks "collecting, will
          // paste on stop" (the tooltip/aria label carries the full wording).
          <span
            data-testid="dictation-batch-hint"
            aria-hidden
            className="bg-primary shadow-glow-accent absolute right-0.5 bottom-0.5 size-1.5 rounded-full"
          />
        ) : null}
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

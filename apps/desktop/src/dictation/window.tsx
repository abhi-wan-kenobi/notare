import { useLingui } from "@lingui/react/macro";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { useEffect, useRef, useState } from "react";

import {
  type DictationStateEvent,
  events as dictationEvents,
} from "@hypr/plugin-dictation";
import { cn } from "@hypr/utils";

import { DictationOrb } from "./orb";

import { useConfigValues } from "~/shared/config";

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
 * The waveform always sits on an explicit token-backed capsule so it remains
 * visible over arbitrary desktop content. `solid` mirrors the floating bar's
 * fallback and fills the rest of the tiny window when transparency fails.
 */
export function DictationOrbWindow({ solid = false }: { solid?: boolean }) {
  const { t } = useLingui();
  const state = useDictationState();
  const { dictation_paste_at_cursor } = useConfigValues([
    "dictation_paste_at_cursor",
  ] as const);
  const pointerStart = useRef<{ x: number; y: number } | null>(null);
  const draggedRef = useRef(false);

  useEffect(() => {
    // The app-level theme provider owns the token set for this webview.

    if (solid) {
      return;
    }

    document.documentElement.style.background = "transparent";
    document.body.style.background = "transparent";
  }, [solid]);

  const dictating = state.phase === "listening" || state.phase === "processing";
  const batchMode = state.mode === "batch";
  const label = dictating
    ? batchMode
      ? dictation_paste_at_cursor
        ? t`Stop dictation and paste the transcript`
        : t`Stop dictation and copy the transcript`
      : t`Stop dictation`
    : t`Start dictation`;

  const handlePointerDown = (event: React.PointerEvent) => {
    if (event.button !== 0) {
      return;
    }
    // Preserve the target's keyboard focus; native panel traits separately
    // prevent activation of the app itself.
    event.preventDefault();
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

  const handleContextMenu = (event: React.MouseEvent) => {
    // Right-click dismisses the idle orb until the next dictation session
    // starts (the host owns the actual hide - see `DictationOrbHost`). While
    // a session is live the orb doubles as the mic indicator, so dismissing
    // it then would leave the mic running with no visual trace - ignore.
    event.preventDefault();
    if (dictating) {
      return;
    }
    void dictationEvents.dictationOrbHideRequested.emit({});
  };

  return (
    <div
      data-testid={solid ? "dictation-window-solid" : "dictation-window-glass"}
      className={cn([
        "text-popover-foreground flex h-screen w-screen items-center justify-center overflow-hidden",
        solid && "bg-popover border-border rounded-xl border",
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
        onContextMenu={handleContextMenu}
        className={cn([
          "bg-popover text-popover-foreground border-border relative flex cursor-pointer items-center justify-center rounded-full border p-0.5 shadow-sm",
          "[--accent-glow:var(--primary)] dark:[--accent-glow:inherit]",
          "focus-visible:ring-ring focus-visible:ring-2 focus-visible:outline-hidden",
        ])}
      >
        <DictationOrb
          phase={state.phase}
          amplitude={state.amplitude}
          size={30}
        />
        {batchMode && dictating ? (
          <span
            data-testid="dictation-batch-hint"
            aria-hidden
            className="bg-popover-foreground absolute right-0.5 bottom-0.5 size-1 rounded-full"
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

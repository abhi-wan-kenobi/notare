import { useLingui } from "@lingui/react/macro";
import { LogicalPosition, LogicalSize } from "@tauri-apps/api/dpi";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { currentMonitor } from "@tauri-apps/api/window";
import { useEffect, useRef, useState } from "react";

import {
  type DictationStateEvent,
  events as dictationEvents,
} from "@hypr/plugin-dictation";
import { cn } from "@hypr/utils";

import {
  DictationOrb,
  type DictationOrbVariant,
  normalizeOrbVariant,
  orbWindowSizeForVariant,
} from "./orb";

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
 * `solid` mirrors the floating bar's fallback: when the OS refused a
 * transparent window, the Rust side loads `/app/dictation?solid=1` and this
 * renders on an opaque rounded surface instead of a bare orb.
 */
export function DictationOrbWindow({ solid = false }: { solid?: boolean }) {
  const { t } = useLingui();
  const state = useDictationState();
  // High-frequency (~30 Hz) mic amplitude, mirrored into a ref so it never
  // re-renders the orb. Not read by the visuals yet; a later PR (WS-E) feeds it
  // to a requestAnimationFrame envelope follower for the orb ring. Subscribing
  // here is what keeps the `DictationAmplitudeEvent` channel live.
  const amplitudeRef = useRef(0);
  useDictationAmplitude(amplitudeRef);
  const { dictation_orb_variant, dictation_paste_at_cursor } = useConfigValues([
    "dictation_orb_variant",
    "dictation_paste_at_cursor",
  ] as const);
  const variant = normalizeOrbVariant(dictation_orb_variant);
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

  // A variant picked while the orb window is hidden (or superseded by a
  // later pick before it could apply) waits here for the next opportunity to
  // sync - see the effect below and `requestOrbWindowSize`'s doc comment.
  const pendingVariantRef = useRef<DictationOrbVariant | null>(null);

  useEffect(() => {
    let cancelled = false;

    void requestOrbWindowSize(variant, () => cancelled).then((applied) => {
      if (!cancelled) {
        pendingVariantRef.current = applied ? null : variant;
      }
    });

    return () => {
      cancelled = true;
    };
  }, [variant]);

  // Flush a resize that was deferred because the window was hidden, the next
  // time a real dictation-session event proves the window is now shown (the
  // orb window is only ever shown for an active/enabled dictation session -
  // see `DictationOrbHost`'s lifecycle effect in `host.tsx`).
  useEffect(() => {
    const pending = pendingVariantRef.current;
    if (!pending) {
      return;
    }

    let cancelled = false;
    void requestOrbWindowSize(pending, () => cancelled).then((applied) => {
      if (!cancelled && applied) {
        pendingVariantRef.current = null;
      }
    });

    return () => {
      cancelled = true;
    };
    // Only the phase transition matters here (it's the signal that the
    // window is now shown); `pendingVariantRef` is a ref, not state, so it
    // is intentionally not a dependency.
  }, [state.phase]);

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
          size={50}
          variant={variant}
          amplitudeRef={amplitudeRef}
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

/**
 * Gate for `applyOrbWindowSize`: a native `setSize`/`setPosition` on a
 * HIDDEN orb window still round-trips through the single-threaded Tauri/
 * WebView2 event loop that every window (including the Settings window)
 * shares, and on Windows that can stall the compositor mid-repaint of
 * whichever webview is currently painting. Picking an orb style in
 * Settings while the orb is off (or between sessions, before it has been
 * shown) must never trigger that resize - the caller (the effects in
 * `DictationOrbWindow`) stashes the variant in `pendingVariantRef` instead
 * and retries once a real dictation-session event proves the window is
 * shown. Returns whether the size was actually applied (or already
 * matched), so the caller knows whether to keep the variant pending.
 */
async function requestOrbWindowSize(
  variant: DictationOrbVariant,
  isCancelled: () => boolean,
): Promise<boolean> {
  try {
    const window = getCurrentWebviewWindow();
    if (!(await window.isVisible())) {
      return false;
    }
    if (isCancelled()) {
      // Superseded by a later variant pick while the visibility check was
      // in flight - let that effect's own call own the resize.
      return true;
    }
    await applyOrbWindowSize(window, variant, isCancelled);
    return true;
  } catch {
    // Not running inside the orb webview (tests/storybook) or the window API
    // is unavailable - keep whatever size the window was created with. This
    // is a terminal failure, not a "try again later" one, so report applied.
    return true;
  }
}

/**
 * The Rust side always creates the orb window at the cobalt chassis size
 * (`ORB_SIZE` in `plugins/dictation/src/orb.rs`); variants that render larger
 * (particles, 1.5x) are resized from here, where the variant setting lives -
 * the same webview-drives-the-window pattern as the floating bar's `update`.
 * The window grows/shrinks around its center so the orb never jumps, and the
 * resulting `Moved` event lets the Rust side persist the adjusted position.
 * The re-centered spot is clamped to the current monitor with the TARGET
 * size (the Rust restore clamp ran at the creation size, so a variant that
 * grows near a screen edge would otherwise end up partly offscreen).
 *
 * Only ever called on a window already confirmed visible
 * (`requestOrbWindowSize`); `isCancelled` lets a later variant pick that
 * arrived while this one was still awaiting IPC round-trips win outright,
 * coalescing a rapid run of picks into a single native resize instead of
 * firing one per click.
 */
async function applyOrbWindowSize(
  window: ReturnType<typeof getCurrentWebviewWindow>,
  variant: DictationOrbVariant,
  isCancelled: () => boolean,
) {
  const target = orbWindowSizeForVariant(variant);
  const scale = await window.scaleFactor();
  const inner = (await window.innerSize()).toLogical(scale);
  if (
    Math.abs(inner.width - target) < 1 &&
    Math.abs(inner.height - target) < 1
  ) {
    return;
  }

  const position = (await window.outerPosition()).toLogical(scale);
  let x = Math.round(position.x + (inner.width - target) / 2);
  let y = Math.round(position.y + (inner.height - target) / 2);

  const monitor = await currentMonitor().catch(() => null);
  if (monitor) {
    const monitorScale = monitor.scaleFactor || 1;
    const monitorX = monitor.position.x / monitorScale;
    const monitorY = monitor.position.y / monitorScale;
    const monitorWidth = monitor.size.width / monitorScale;
    const monitorHeight = monitor.size.height / monitorScale;
    x = clamp(x, monitorX, monitorX + monitorWidth - target);
    y = clamp(y, monitorY, monitorY + monitorHeight - target);
  }

  if (isCancelled()) {
    return;
  }

  await window.setSize(new LogicalSize(target, target));
  await window.setPosition(new LogicalPosition(Math.round(x), Math.round(y)));
}

function clamp(value: number, min: number, max: number): number {
  return Math.min(Math.max(value, min), Math.max(min, max));
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

/**
 * Subscribe to the high-frequency (~30 Hz) `DictationAmplitudeEvent` channel
 * and mirror the newest sample into `target.current`. Deliberately writes a
 * ref and never calls `setState`: at 30 Hz a state update would re-render the
 * orb on every frame. This is the thin, separate companion to
 * `useDictationState` (10 Hz, phase/mode -> renders). A later PR reads this ref
 * from a requestAnimationFrame envelope follower to drive the orb ring
 * smoothly; for now it only keeps the latest amplitude available at zero render
 * cost. `target` is typed structurally so it accepts a `useRef` object across
 * React versions.
 */
function useDictationAmplitude(target: { current: number }) {
  useEffect(() => {
    const channel = dictationEvents.dictationAmplitudeEvent;
    if (!channel) {
      // Partially-mocked plugin (tests/storybook): no 30 Hz stream to mirror,
      // so the orb visuals fall back to the 10 Hz `amplitude` prop.
      return;
    }
    let cancelled = false;
    let unlisten: (() => void) | null = null;

    void channel
      .listen((event) => {
        if (!cancelled) {
          target.current = event.payload.amplitude;
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
  }, [target]);
}

import { useLingui } from "@lingui/react/macro";
import {
  AppWindowIcon,
  CaptionsIcon,
  CaptionsOffIcon,
  SquareIcon,
} from "lucide-react";
import { useEffect, useState } from "react";

import {
  events as windowsEvents,
  type FloatingBarState,
} from "@hypr/plugin-windows";
import { cn } from "@hypr/utils";

import { DictationOrb } from "~/dictation/orb";

/**
 * Webview-based floating meeting widget used on EVERY platform.
 *
 * The Rust side of the windows plugin creates a small always-on-top webview
 * window pointing at `/app/floating` and streams `FloatingBarStateEvent`s to
 * it. Buttons talk back to the main window through plugin events
 * (`floatingBarStop`, `floatingBarOpenMain`, `floatingBarSettingsChange`).
 *
 * macOS hosts this same webview inside a non-activating NSPanel. The old
 * Swift-only renderer is no longer used. The compact control keeps the
 * original timer/waveform layout with an expandable transcript beneath it.
 * `/app/floating?solid=1` supplies an opaque fallback when transparency is
 * unavailable.
 */

export function FloatingBarWindow({ solid = false }: { solid?: boolean }) {
  const state = useFloatingBarState();

  useEffect(() => {
    if (solid) {
      return;
    }

    document.documentElement.style.background = "transparent";
    document.body.style.background = "transparent";
  }, [solid]);

  // The widget colors come from the shared theme tokens; mirror the main
  // window's applied scheme onto this webview's root element.
  useEffect(() => {
    if (!state) {
      return;
    }

    document.documentElement.classList.toggle(
      "dark",
      state.colorScheme === "dark",
    );
  }, [state?.colorScheme]);

  if (!state) {
    return null;
  }

  return <FloatingBarContent state={state} solid={solid} />;
}

function useFloatingBarState() {
  const [state, setState] = useState<FloatingBarState | null>(null);

  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | null = null;

    void windowsEvents.floatingBarStateEvent
      .listen((event) => {
        if (!cancelled) {
          setState(event.payload.state);
        }
      })
      .then((next) => {
        if (cancelled) {
          next();
          return;
        }

        unlisten = next;
        // Ask the main window to push the current state: this window loads
        // after recording starts, so the initial updates were already sent.
        void windowsEvents.floatingBarReady.emit({});
      })
      .catch((error) => {
        console.error(
          "[meeting-float] failed to listen for floating bar state",
          error,
        );
      });

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  return state;
}

export function FloatingBarContent({
  state,
  solid = false,
}: {
  state: FloatingBarState;
  solid?: boolean;
}) {
  const { t } = useLingui();

  const isError = state.status === "error";
  const captionsVisible = !state.liveCaptionMinimized;

  // Map the floating bar status to DictationPhase:
  // - "error" status maps to "error" phase.
  // - "recording" status maps to "listening" phase (as the bar doesn't have an idle or processing phase).
  const phase = isError ? "error" : "listening";

  const handleStop = () => {
    void windowsEvents.floatingBarStop.emit({});
  };
  const handleOpenMain = () => {
    void windowsEvents.floatingBarOpenMain.emit({});
  };
  const handleToggleCaptions = () => {
    void windowsEvents.floatingBarSettingsChange.emit({
      floatingBarOpacity: null,
      liveCaptionOpacity: null,
      liveCaptionWidth: null,
      liveCaptionLineCount: null,
      liveCaptionPosition: null,
      liveCaptionMinimized: captionsVisible,
    });
  };

  return (
    <div
      data-testid={solid ? "floating-bar-solid" : "floating-bar-glass"}
      className={cn([
        "text-foreground flex h-screen w-screen flex-col overflow-hidden",
        solid && "bg-background border-border rounded-xl border",
      ])}
    >
      <div
        data-tauri-drag-region
        className={cn([
          "group/bar mx-auto mt-1 flex h-[40px] shrink-0 items-center gap-1.5 px-1.5",
          solid
            ? "border-border/80 border-b"
            : "border-border/70 rounded-full border backdrop-blur-xl",
        ])}
        style={
          solid
            ? undefined
            : {
                backgroundColor: `hsl(var(--popover) / ${
                  Number.isFinite(state.opacity)
                    ? Math.min(Math.max(state.opacity, 0), 1)
                    : 1
                })`,
              }
        }
      >
        <button
          type="button"
          aria-label={t`Stop recording`}
          title={t`Stop recording`}
          onPointerDown={(event) => event.preventDefault()}
          onClick={handleStop}
          className="group/orb relative shrink-0 rounded-full focus-visible:ring-2 focus-visible:outline-hidden"
        >
          <DictationOrb phase={phase} amplitude={state.amplitude} size={30} />
          <span className="bg-background/90 absolute inset-0 hidden items-center justify-center rounded-full group-hover/orb:flex">
            <SquareIcon aria-hidden className="size-3 fill-current" />
          </span>
        </button>
        {!isError && (
          <span
            aria-hidden
            className="bg-foreground/60 animate-orb-pulse size-1.5 shrink-0 rounded-full motion-reduce:animate-none"
          />
        )}
        <RecordingMeter
          elapsedSeconds={state.elapsedSeconds}
          wordCount={state.wordCount}
        />
        <span
          data-tauri-drag-region
          className="max-w-[120px] min-w-0 truncate text-xs font-medium"
        >
          {state.title}
        </span>
        <div
          className={cn([
            "flex shrink-0 items-center gap-0.5",
            "opacity-60 transition-opacity duration-(--motion-duration-state)",
            "group-hover/bar:opacity-100 focus-within:opacity-100",
          ])}
        >
          {state.liveCaptionToggleVisible ? (
            <FloatingBarButton
              label={captionsVisible ? t`Hide captions` : t`Show captions`}
              onClick={handleToggleCaptions}
              active={captionsVisible}
            >
              {captionsVisible ? (
                <CaptionsIcon className="size-4" />
              ) : (
                <CaptionsOffIcon className="size-4" />
              )}
            </FloatingBarButton>
          ) : null}
          <FloatingBarButton
            label={t`Open main window`}
            onClick={handleOpenMain}
          >
            <AppWindowIcon className="size-4" />
          </FloatingBarButton>
        </div>
      </div>
      {captionsVisible ? (
        <FloatingBarCaptions
          bubbles={state.transcriptBubbles}
          lineCount={state.liveCaptionLineCount}
          solid={solid}
        />
      ) : null}
    </div>
  );
}

/** Weighted timer and optional word count, with stable tabular digits. */
function RecordingMeter({
  elapsedSeconds,
  wordCount,
}: {
  elapsedSeconds: number;
  wordCount: number;
}) {
  const { t } = useLingui();
  const elapsed = formatWeightedElapsed(elapsedSeconds);

  return (
    <span
      data-testid="recording-meter"
      data-tauri-drag-region
      className="flex shrink-0 items-baseline gap-1.5"
    >
      <span
        // `tabular-nums` is mandatory on timers (§103): proportional digits
        // reflow the whole bar on every tick.
        className="text-muted-foreground text-[13px] tabular-nums"
        aria-label={t`Recording time`}
      >
        {elapsed.dim}
        <span className="text-foreground font-semibold">{elapsed.bold}</span>
      </span>
      {wordCount > 0 ? (
        <span
          data-testid="recording-word-count"
          className="text-muted-foreground/70 text-[11px] tabular-nums"
        >
          {wordCount}
        </span>
      ) : null}
    </span>
  );
}

function FloatingBarButton({
  active = false,
  children,
  label,
  onClick,
}: {
  active?: boolean;
  children: React.ReactNode;
  label: string;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      aria-label={label}
      title={label}
      aria-pressed={active}
      onClick={onClick}
      className={cn([
        "flex size-7 shrink-0 items-center justify-center rounded-full",
        "transition-colors duration-(--motion-duration-state)",
        "text-muted-foreground hover:bg-accent hover:text-foreground",
        "focus-visible:ring-ring focus-visible:ring-2 focus-visible:outline-hidden",
        active && "bg-accent text-foreground",
      ])}
    >
      {children}
    </button>
  );
}

const VISIBLE_CAPTION_BUBBLES = 3;

/**
 * Expandable caption bubble: last lines of the live transcript on a raised
 * glass surface. The newest line carries a cobalt live edge (glow =
 * information); older lines commit to muted text (§3a two-tone mechanics).
 */
function FloatingBarCaptions({
  bubbles,
  lineCount,
  solid,
}: {
  bubbles: FloatingBarState["transcriptBubbles"];
  lineCount: number;
  solid: boolean;
}) {
  const { t } = useLingui();
  const visibleBubbles = (bubbles ?? []).slice(-VISIBLE_CAPTION_BUBBLES);

  return (
    <div
      data-testid="floating-bar-captions"
      className={cn([
        "flex min-h-0 flex-1 flex-col justify-end overflow-hidden px-3 pt-1 pb-1.5",
        solid
          ? "border-border/80 border-t"
          : "border-border/70 mt-1 rounded-[10px] border backdrop-blur-xl",
      ])}
      style={
        solid ? undefined : { backgroundColor: "hsl(var(--popover) / 0.92)" }
      }
    >
      {visibleBubbles.length === 0 ? (
        <p className="text-muted-foreground animate-orb-pulse text-xs motion-reduce:animate-none">
          {t`Listening...`}
        </p>
      ) : (
        visibleBubbles.map((bubble, index) => {
          const isLatest = index === visibleBubbles.length - 1;

          return (
            <p
              key={bubble.id}
              className={cn([
                "text-xs leading-[22px]",
                isLatest
                  ? [
                      "border-primary/70 shrink-0 border-l-2 pl-2",
                      lineCount > 1 ? "line-clamp-2" : "truncate",
                    ]
                  : [
                      "text-muted-foreground truncate border-l-2 border-transparent pl-2",
                    ],
              ])}
            >
              <span className="font-medium">{bubble.speakerLabel}</span>
              {": "}
              {bubble.text}
            </p>
          );
        })
      )}
    </div>
  );
}

/**
 * Split an elapsed-seconds count into the weighted timer's dim prefix and bold
 * seconds (docs/DESIGN-DIRECTION.md §104: "dim `hh:mm`, bold `ss`"). Exported
 * for tests — the hour rollover and the zero-pad are easy to get subtly wrong
 * and are not otherwise observable without driving a live recording.
 */
export function formatWeightedElapsed(totalSeconds: number): {
  dim: string;
  bold: string;
} {
  const safe = Number.isFinite(totalSeconds)
    ? Math.max(0, Math.floor(totalSeconds))
    : 0;
  const hours = Math.floor(safe / 3600);
  const minutes = Math.floor((safe % 3600) / 60);
  const seconds = safe % 60;
  const bold = String(seconds).padStart(2, "0");

  if (hours > 0) {
    return { dim: `${hours}:${String(minutes).padStart(2, "0")}:`, bold };
  }

  return { dim: `${String(minutes).padStart(2, "0")}:`, bold };
}

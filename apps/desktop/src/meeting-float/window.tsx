import { useLingui } from "@lingui/react/macro";
import {
  AppWindowIcon,
  CaptionsIcon,
  CaptionsOffIcon,
  Maximize2Icon,
  Minimize2Icon,
  SquareIcon,
} from "lucide-react";
import { useEffect, useRef, useState } from "react";

import {
  events as windowsEvents,
  type FloatingBarState,
} from "@hypr/plugin-windows";
import { cn } from "@hypr/utils";

import {
  DictationOrb,
  normalizeOrbVariant,
  ORB_VARIANT_SCALE,
  type DictationOrbVariant,
} from "~/dictation/orb";
import { useConfigValues } from "~/shared/config";

/**
 * Webview-based floating meeting widget used on Windows/Linux.
 *
 * macOS renders a native NSPanel instead; on other platforms the Rust side of
 * the windows plugin creates a small always-on-top webview window pointing at
 * `/app/floating` and streams `FloatingBarStateEvent`s to it. Buttons talk
 * back to the main window through the same plugin events the native macOS bar
 * uses (`floatingBarStop`, `floatingBarOpenMain`, `floatingBarSettingsChange`).
 *
 * Design: docs/DESIGN-DIRECTION.md §3b — the orb inside a glass bar, with an
 * expandable caption bubble underneath. When the OS window could not be
 * created with transparency (known Windows limitation), the Rust side loads
 * `/app/floating?solid=1` and this component renders the solid-surface
 * variant instead of glass.
 */
/**
 * Look of the meeting floating bar, selected by the `meeting_bar_theme`
 * setting: "notare" (the default glass orb bar) or "classic" (the compact
 * parchment/olive pill ported from the earlier native macOS NSPanel,
 * `FloatingBarView.swift`). Mirrors `dictation_orb_variant`'s conventions.
 */
export type MeetingBarTheme = "notare" | "classic";

export const DEFAULT_MEETING_BAR_THEME: MeetingBarTheme = "notare";

/** Map whatever the settings store holds onto a known bar theme. */
export function normalizeMeetingBarTheme(
  raw: string | undefined,
): MeetingBarTheme {
  return raw === "classic" ? "classic" : "notare";
}

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
  const { dictation_orb_variant, meeting_bar_theme } = useConfigValues([
    "dictation_orb_variant",
    "meeting_bar_theme",
  ] as const);
  const theme = normalizeMeetingBarTheme(meeting_bar_theme);

  if (theme === "classic") {
    return <ClassicFloatingBar state={state} solid={solid} />;
  }

  const isError = state.status === "error";
  const captionsVisible = !state.liveCaptionMinimized;

  const variant = normalizeOrbVariant(dictation_orb_variant);

  // Map the floating bar status to DictationPhase:
  // - "error" status maps to "error" phase.
  // - "recording" status maps to "listening" phase (as the bar doesn't have an idle or processing phase).
  const phase = isError ? "error" : "listening";

  const orbSize = getOrbSize(variant);

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
          "group/bar flex h-[52px] shrink-0 items-center gap-2.5 px-2.5",
          solid
            ? "border-border/80 border-b"
            : "border-border/70 rounded-full border backdrop-blur-xl",
        ])}
        style={
          solid
            ? undefined
            : {
                backgroundColor: `hsl(var(--popover) / ${state.opacity})`,
              }
        }
      >
        <span data-tauri-drag-region className="pointer-events-none shrink-0">
          <DictationOrb
            phase={phase}
            amplitude={state.amplitude}
            size={orbSize}
            variant={variant}
          />
        </span>
        {!isError && (
          <span
            aria-hidden
            className="bg-rec animate-orb-pulse shadow-glow-rec size-1.5 shrink-0 rounded-full motion-reduce:animate-none"
          />
        )}
        <span
          data-tauri-drag-region
          className="min-w-0 flex-1 truncate text-[13px] font-medium"
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
          <FloatingBarButton label={t`Stop recording`} onClick={handleStop}>
            <SquareIcon className="fill-rec text-rec size-3.5" />
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
  const visibleBubbles = bubbles.slice(-VISIBLE_CAPTION_BUBBLES);

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

function getOrbSize(variant: DictationOrbVariant): number {
  const scale = ORB_VARIANT_SCALE[variant] ?? 1;
  return Math.round(32 / scale);
}

/**
 * "Classic" meeting bar: a faithful React port of the earlier native macOS
 * `FloatingBarView` (a compact parchment/olive pill with a red accent and a
 * 5-bar "DancingBars" waveform). The same webview window hosts it as the
 * Notare bar - only the React surface differs - so it runs cross-platform.
 *
 * Palette and waveform math are sampled from `FloatingBarView.swift`
 * (surface :347-361, accent :415-417, DancingBars :845-880). The expanded
 * transcript reuses `FloatingBarCaptions` (the Notare caption bubble) so the
 * live transcript stays identical across themes.
 */
function ClassicFloatingBar({
  state,
  solid = false,
}: {
  state: FloatingBarState;
  solid?: boolean;
}) {
  const { t } = useLingui();
  const isError = state.status === "error";
  const expanded = !state.liveCaptionMinimized;
  const palette = classicPalette(state.colorScheme);

  // Surface opacity mirrors the native primary surface: floatingBarOpacity *
  // 0.82 (`primarySurfaceOpacity` in FloatingBarView.swift). `state.opacity`
  // carries the persisted `floating_bar_opacity`.
  const surfaceAlpha = Math.min(Math.max(state.opacity * 0.82, 0), 1);
  const surfaceColor = withAlpha(palette.surface, surfaceAlpha);

  const handleStop = () => {
    void windowsEvents.floatingBarStop.emit({});
  };
  const handleToggleExpand = () => {
    void windowsEvents.floatingBarSettingsChange.emit({
      floatingBarOpacity: null,
      liveCaptionOpacity: null,
      liveCaptionWidth: null,
      liveCaptionLineCount: null,
      liveCaptionPosition: null,
      liveCaptionMinimized: expanded,
    });
  };

  return (
    <div
      data-testid={solid ? "classic-bar-solid" : "classic-bar"}
      data-classic-expanded={expanded ? "true" : undefined}
      className={cn([
        "flex h-screen w-screen flex-col items-stretch justify-start overflow-hidden",
        solid && "bg-background border-border rounded-xl border",
      ])}
    >
      <div
        data-tauri-drag-region
        className={cn([
          "flex items-center gap-[3px] px-1",
          expanded ? "h-[52px]" : "mx-auto mt-1 h-[40px]",
        ])}
        style={
          solid
            ? undefined
            : {
                backgroundColor: surfaceColor,
                borderRadius: 9999,
                boxShadow: `inset 0 0 0 0.5px ${withAlpha(
                  palette.content,
                  palette.strokeOuter,
                )}, inset 0 0 0 1.5px ${withAlpha(
                  palette.content,
                  palette.strokeInner,
                )}`,
              }
        }
      >
        {expanded ? (
          <span
            data-tauri-drag-region
            className="min-w-0 flex-1 truncate px-1.5 text-[13px] font-semibold"
            style={{ color: withAlpha(palette.content, 1) }}
          >
            {state.title}
          </span>
        ) : null}
        <ClassicStopButton
          amplitude={state.amplitude}
          isError={isError}
          palette={palette}
          showLabel={state.liveCaptionToggleVisible ? 62 : 68}
          onStop={handleStop}
        />
        {state.liveCaptionToggleVisible ? (
          <ClassicIconButton
            label={
              expanded ? t`Collapse live transcript` : t`Expand live transcript`
            }
            palette={palette}
            onClick={handleToggleExpand}
            data-testid="classic-expand-button"
          >
            {expanded ? (
              <Minimize2Icon className="size-3.5" />
            ) : (
              <Maximize2Icon className="size-3.5" />
            )}
          </ClassicIconButton>
        ) : null}
      </div>
      {expanded ? (
        <FloatingBarCaptions
          bubbles={state.transcriptBubbles}
          lineCount={state.liveCaptionLineCount}
          solid={solid}
        />
      ) : null}
    </div>
  );
}

interface ClassicPalette {
  surface: RGB;
  content: RGB;
  accent: RGB;
  errorAccent: RGB;
  /** Alpha overlays of `content` for strokes / hover fills, per color scheme. */
  strokeOuter: number;
  strokeInner: number;
  controlHover: number;
  accentHover: number;
}

type RGB = readonly [number, number, number];

function classicPalette(
  colorScheme: FloatingBarState["colorScheme"],
): ClassicPalette {
  if (colorScheme === "dark") {
    return {
      // FloatingBarView.swift surfaceColor (dark) :348-349
      surface: [110, 112, 102],
      content: [255, 255, 255],
      // normalAccentColor :415-417, errorAccentColor :411-413
      accent: [255, 51, 77],
      errorAccent: [255, 64, 61],
      // primaryContentColor-derived opacities (dark) :380-393
      strokeOuter: 0.14,
      strokeInner: 0.28,
      controlHover: 0.08,
      accentHover: 0.18,
    };
  }
  return {
    // surfaceColor (light) :352
    surface: [219, 217, 209],
    // primaryContentColor (light) :376
    content: [31, 28, 26],
    accent: [255, 51, 77],
    errorAccent: [255, 64, 61],
    // primaryContentColor-derived opacities (light) :380-393
    strokeOuter: 0.12,
    strokeInner: 0.18,
    controlHover: 0.07,
    accentHover: 0.18,
  };
}

function withAlpha([r, g, b]: RGB, alpha: number): string {
  return `rgba(${r}, ${g}, ${b}, ${alpha})`;
}

/**
 * Stop capsule: shows the 5-bar DancingBars waveform while recording, an
 * ErrorMark when the bar is in the error state, and a "Stop" label + square
 * on hover. Ported from `FloatingBarView.swift` `audioControl` :299-334.
 */
function ClassicStopButton({
  amplitude,
  isError,
  palette,
  showLabel,
  onStop,
}: {
  amplitude: number;
  isError: boolean;
  palette: ClassicPalette;
  showLabel: number;
  onStop: () => void;
}) {
  const { t } = useLingui();
  const [hovered, setHovered] = useState(false);
  const accent = isError ? palette.errorAccent : palette.accent;

  return (
    <button
      type="button"
      aria-label={t`Stop recording`}
      title={t`Stop recording`}
      data-testid="classic-stop-button"
      onClick={onStop}
      onMouseEnter={() => setHovered(true)}
      onMouseLeave={() => setHovered(false)}
      className="flex shrink-0 items-center justify-center gap-1.5 rounded-full transition-colors duration-(--motion-duration-state)"
      style={{
        minWidth: showLabel,
        height: 30,
        paddingLeft: 8,
        paddingRight: 8,
        backgroundColor: hovered
          ? withAlpha(palette.accent, palette.accentHover)
          : withAlpha(palette.content, palette.controlHover),
        color: withAlpha(accent, 1),
      }}
    >
      {isError ? (
        <ClassicErrorMark color={accent} />
      ) : hovered ? (
        <>
          <SquareIcon className="size-2.5 fill-current" />
          <span className="text-[12px] font-semibold">{t`Stop`}</span>
        </>
      ) : (
        <DancingBars color={accent} amplitude={amplitude} />
      )}
    </button>
  );
}

function ClassicIconButton({
  label,
  palette,
  onClick,
  children,
  "data-testid": testId,
}: {
  label: string;
  palette: ClassicPalette;
  onClick: () => void;
  children: React.ReactNode;
  "data-testid"?: string;
}) {
  const [hovered, setHovered] = useState(false);
  return (
    <button
      type="button"
      aria-label={label}
      title={label}
      data-testid={testId}
      onClick={onClick}
      onMouseEnter={() => setHovered(true)}
      onMouseLeave={() => setHovered(false)}
      className="flex size-[30px] shrink-0 items-center justify-center rounded-full transition-colors duration-(--motion-duration-state)"
      style={{
        backgroundColor: hovered
          ? withAlpha(palette.content, palette.controlHover)
          : "transparent",
        color: withAlpha(palette.content, 1),
      }}
    >
      {children}
    </button>
  );
}

/**
 * 5-bar amplitude waveform. Ported from `DancingBars` in
 * `FloatingBarView.swift` :845-880: each bar's height is a sine wave whose
 * drive tracks `amplitude`, with a center-weighted envelope so the middle
 * bars dance tallest. `prefers-reduced-motion` freezes the bars at a mid
 * level (matching the orb previews' reduced-motion behavior).
 */
function DancingBars({ color, amplitude }: { color: RGB; amplitude: number }) {
  const ampRef = useRef(amplitude);
  ampRef.current = amplitude;
  const [heights, setHeights] = useState<number[]>(() =>
    Array.from({ length: CLASSIC_BAR_COUNT }, () => CLASSIC_BAR_MIN_HEIGHT),
  );
  const reduced = usePrefersReducedMotion();

  useEffect(() => {
    if (reduced) {
      setHeights(
        Array.from(
          { length: CLASSIC_BAR_COUNT },
          () => CLASSIC_BAR_MAX_HEIGHT * 0.5,
        ),
      );
      return;
    }

    let raf = 0;
    const start = performance.now();
    const tick = (now: number) => {
      const t = (now - start) / 1000;
      setHeights(classicBarHeights(t, ampRef.current));
      raf = requestAnimationFrame(tick);
    };
    raf = requestAnimationFrame(tick);
    return () => {
      cancelAnimationFrame(raf);
    };
  }, [reduced]);

  return (
    <span
      data-testid="classic-dancing-bars"
      aria-hidden
      className="flex h-[20px] items-center"
      style={{ gap: 2 }}
    >
      {heights.map((height, index) => (
        <span
          key={index}
          className="block"
          style={{
            width: 3,
            height,
            borderRadius: 9999,
            backgroundColor: withAlpha(color, 1),
          }}
        />
      ))}
    </span>
  );
}

const CLASSIC_BAR_COUNT = 5;
const CLASSIC_BAR_MIN_HEIGHT = 4;
const CLASSIC_BAR_MAX_HEIGHT = 20;

function classicBarHeights(time: number, amplitude: number): number[] {
  const normalized = Math.min(Math.max(amplitude, 0), 1);
  const center = (CLASSIC_BAR_COUNT - 1) / 2;
  return Array.from({ length: CLASSIC_BAR_COUNT }, (_, index) => {
    const distance = Math.abs(index - center) / Math.max(center, 1);
    const envelope = 1 - distance * 0.42;
    const phase = time * 8.5 + index * 0.68;
    const wave = Math.sin(phase) * 0.5 + 0.5;
    const drive = 0.4 + normalized * 0.9;
    const height =
      CLASSIC_BAR_MAX_HEIGHT * drive * envelope * (0.4 + wave * 0.6);
    return Math.min(
      Math.max(height, CLASSIC_BAR_MIN_HEIGHT),
      CLASSIC_BAR_MAX_HEIGHT,
    );
  });
}

/** Native `ErrorMark` :830-843: a red capsule above a red dot. */
function ClassicErrorMark({ color }: { color: RGB }) {
  return (
    <span
      data-testid="classic-error-mark"
      aria-hidden
      className="flex flex-col items-center"
      style={{ gap: 1.5 }}
    >
      <span
        style={{
          width: 3.2,
          height: 8,
          borderRadius: 9999,
          backgroundColor: withAlpha(color, 1),
        }}
      />
      <span
        style={{
          width: 3.2,
          height: 3.2,
          borderRadius: 9999,
          backgroundColor: withAlpha(color, 1),
        }}
      />
    </span>
  );
}

function usePrefersReducedMotion(): boolean {
  const [reduced, setReduced] = useState(false);
  useEffect(() => {
    if (typeof window.matchMedia !== "function") {
      return;
    }
    const query = window.matchMedia("(prefers-reduced-motion: reduce)");
    const update = () => setReduced(query.matches);
    update();
    query.addEventListener("change", update);
    return () => {
      query.removeEventListener("change", update);
    };
  }, []);
  return reduced;
}

import { useEffect, useRef } from "react";

import type { DictationPhase } from "@hypr/plugin-dictation";

/**
 * "Cobalt Halo" orb variant: twin concentric hairline rings wrapped in a
 * soft, canvas-drawn bloom - the calm, minimal default look. Everything is
 * painted on a 2D canvas (NO CSS `blur`/`drop-shadow`): the glow is a real
 * radial gradient composited additively, so it renders identically in the
 * transparent orb webview and the `?solid=1` opaque fallback and never leaks
 * outside the canvas box.
 *
 * Palette matches the "cobalt" meeting orb (the dark token set the orb window
 * forces): cobalt `--primary` #4D5CFF, iridescent rim `--accent-glow` #6EE7FF,
 * `--accent-glow-end` #A78BFA.
 *
 * Phase mapping:
 * - idle: two faint static hairline rings + a whisper of bloom (no motion, so
 *   the rAF loop is not even started - see the component below).
 * - listening: rings brighten and the bloom swells with the live mic
 *   amplitude (fast-attack / slow-decay envelope, mirroring `particle-orb`),
 *   and a short bright highlight arc orbits the outer ring.
 * - processing: a gentle constant breath keeps the halo alive while the final
 *   segments flush (the variant-agnostic `ProcessingRing` spinner in `orb.tsx`
 *   overlays the actual spinner, same as every sibling).
 * - error: desaturated slate rings, static, with the shared destructive badge
 *   dot bottom-right (matching the other canvas orbs).
 *
 * `prefers-reduced-motion` renders a single static frame per phase with no
 * animation loop; the rAF loop only ever runs for the two genuinely animated
 * phases (listening/processing), so an idle or error orb costs zero frames.
 */

type Rgb = readonly [number, number, number];

/** Cobalt `--primary` (dark), the ring/core hue. */
const COBALT: Rgb = [77, 92, 255];
/** `--accent-glow` iridescent rim start. */
const GLOW: Rgb = [110, 231, 255];
/** `--accent-glow-end` iridescent rim end. */
const GLOW_END: Rgb = [167, 139, 250];
/** Desaturated slate for the error look (rings only; red lives in the badge). */
const ERROR: Rgb = [150, 156, 176];

const TAU = Math.PI * 2;

/** Keeps "listening" visibly alive during brief speech pauses. */
const LISTENING_FLOOR = 0.06;

interface PhaseLook {
  /** Constant intensity target (listening uses the live amplitude instead). */
  intensityTarget: number;
  /** Highlight-arc orbit speed per frame (0 = no orbiting arc). */
  spin: number;
  /** Bloom strength multiplier for this phase. */
  bloom: number;
  /** Extra baseline ring opacity so a phase can read "lit" at zero amplitude. */
  ringBoost: number;
}

const PHASE_LOOKS: Record<DictationPhase, PhaseLook> = {
  idle: { intensityTarget: 0, spin: 0, bloom: 0.7, ringBoost: 0 },
  listening: { intensityTarget: 0, spin: 0.02, bloom: 1, ringBoost: 0.12 },
  processing: {
    intensityTarget: 0.32,
    spin: 0.03,
    bloom: 0.9,
    ringBoost: 0.06,
  },
  error: { intensityTarget: 0, spin: 0, bloom: 0, ringBoost: 0 },
};

interface RendererState {
  intensity: number;
  angle: number;
}

function clamp01(value: number): number {
  if (!Number.isFinite(value)) {
    return 0;
  }
  return Math.min(Math.max(value, 0), 1);
}

function mixRgb(a: Rgb, b: Rgb, t: number): Rgb {
  return [
    Math.round(a[0] + (b[0] - a[0]) * t),
    Math.round(a[1] + (b[1] - a[1]) * t),
    Math.round(a[2] + (b[2] - a[2]) * t),
  ];
}

function rgba(color: Rgb, alpha: number): string {
  return `rgba(${color[0]}, ${color[1]}, ${color[2]}, ${clamp01(alpha).toFixed(
    3,
  )})`;
}

/** Advance the amplitude envelope + orbit angle (fast attack, slow decay). */
function stepState(
  state: RendererState,
  phase: DictationPhase,
  amplitude: number,
) {
  const look = PHASE_LOOKS[phase];
  const target =
    phase === "listening"
      ? Math.max(LISTENING_FLOOR, clamp01(amplitude))
      : look.intensityTarget;

  // Reference envelope shared with `particle-orb`: attack 0.15, decay 0.04.
  state.intensity +=
    (target - state.intensity) * (target > state.intensity ? 0.15 : 0.04);

  state.angle += look.spin * (1 + state.intensity * 1.5);
  if (state.angle > TAU) {
    state.angle -= TAU;
  }
}

/** Paint one frame: bloom, twin hairline rings, optional highlight arc. */
function drawFrame(
  ctx: CanvasRenderingContext2D,
  size: number,
  phase: DictationPhase,
  state: RendererState,
) {
  const look = PHASE_LOOKS[phase];
  const isError = phase === "error";
  const { intensity } = state;
  const center = size / 2;

  ctx.clearRect(0, 0, size, size);

  const outerR = Math.max(1, center * 0.84);
  const innerR = Math.max(0.5, center * 0.52);
  const ringW = Math.max(0.6, size * 0.02) * (1 + intensity * 0.8);

  // Canvas-drawn bloom: an additive radial gradient (no CSS blur/shadow).
  if (!isError && look.bloom > 0) {
    const bloomR = Math.max(1, center * (0.5 + intensity * 0.55) * look.bloom);
    const strength = (0.1 + intensity * 0.45) * look.bloom;
    const gradient = ctx.createRadialGradient(
      center,
      center,
      0,
      center,
      center,
      bloomR,
    );
    gradient.addColorStop(0, rgba(mixRgb(GLOW, GLOW_END, 0.3), strength));
    gradient.addColorStop(0.5, rgba(COBALT, strength * 0.5));
    gradient.addColorStop(1, rgba(COBALT, 0));

    ctx.globalCompositeOperation = "lighter";
    ctx.fillStyle = gradient;
    ctx.beginPath();
    ctx.arc(center, center, bloomR, 0, TAU);
    ctx.fill();
    ctx.globalCompositeOperation = "source-over";
  }

  const outerColor = isError ? ERROR : GLOW;
  const innerColor = isError ? ERROR : COBALT;
  const baseOpacity = isError ? 0.5 : 0.32 + intensity * 0.55 + look.ringBoost;

  // Outer hairline ring.
  ctx.lineWidth = ringW;
  ctx.strokeStyle = rgba(outerColor, baseOpacity);
  ctx.beginPath();
  ctx.arc(center, center, outerR, 0, TAU);
  ctx.stroke();

  // Inner hairline ring (thinner, cobalt).
  ctx.lineWidth = Math.max(0.5, ringW * 0.8);
  ctx.strokeStyle = rgba(innerColor, baseOpacity * 0.9);
  ctx.beginPath();
  ctx.arc(center, center, innerR, 0, TAU);
  ctx.stroke();

  // Orbiting highlight arc: the motion channel on animated phases.
  if (!isError && look.spin > 0) {
    const arcLen = Math.PI * (0.2 + intensity * 0.25);
    ctx.lineWidth = ringW * 1.3;
    ctx.lineCap = "round";
    ctx.strokeStyle = rgba(
      mixRgb(GLOW, [255, 255, 255], 0.45),
      0.45 + intensity * 0.5,
    );
    ctx.beginPath();
    ctx.arc(center, center, outerR, state.angle, state.angle + arcLen);
    ctx.stroke();
    ctx.lineCap = "butt";
  }
}

function prefersReducedMotion(): boolean {
  return (
    typeof window !== "undefined" &&
    typeof window.matchMedia === "function" &&
    window.matchMedia("(prefers-reduced-motion: reduce)").matches
  );
}

/** Phases that actually animate; every other phase is a single static frame. */
function isAnimatedPhase(phase: DictationPhase): boolean {
  return phase === "listening" || phase === "processing";
}

export function CobaltHaloOrb({
  phase,
  amplitude,
  size,
}: {
  phase: DictationPhase;
  amplitude: number;
  size: number;
}) {
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const phaseRef = useRef(phase);
  const amplitudeRef = useRef(amplitude);
  phaseRef.current = phase;
  amplitudeRef.current = amplitude;

  const reducedMotion = prefersReducedMotion();
  // The rAF loop only runs for genuinely animated phases; idle/error (and any
  // phase under reduced motion) render a single static frame and start no
  // loop, so a resting orb costs zero frames. `animate` and `phase` are effect
  // dependencies so entering/leaving an animated phase starts/stops the loop
  // and a static phase redraws its frame.
  const animate = !reducedMotion && isAnimatedPhase(phase);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) {
      return;
    }
    const ctx = canvas.getContext("2d");
    if (!ctx) {
      // jsdom / lost context: leave the (aria-hidden) canvas empty.
      return;
    }

    const dpr = Math.min(window.devicePixelRatio || 1, 2);
    canvas.width = Math.round(size * dpr);
    canvas.height = Math.round(size * dpr);
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);

    const state: RendererState = { intensity: 0, angle: -Math.PI / 2 };

    if (!animate) {
      // Static single frame for the current phase. A reduced-motion listening
      // orb still reads as "hot" via a fixed mid-level intensity.
      state.intensity =
        reducedMotion && phaseRef.current === "listening" ? 0.4 : 0;
      drawFrame(ctx, size, phaseRef.current, state);
      return;
    }

    let raf = 0;
    const tick = () => {
      stepState(state, phaseRef.current, amplitudeRef.current);
      drawFrame(ctx, size, phaseRef.current, state);
      raf = requestAnimationFrame(tick);
    };
    raf = requestAnimationFrame(tick);

    return () => {
      cancelAnimationFrame(raf);
    };
  }, [size, reducedMotion, animate, phase]);

  return (
    <span
      className="relative inline-flex shrink-0"
      style={{ width: size, height: size }}
    >
      <canvas
        ref={canvasRef}
        data-testid="dictation-cobalt-halo-orb"
        aria-hidden
        style={{ width: size, height: size }}
      />
      {phase === "error" && (
        <span
          data-testid="dictation-cobalt-halo-error-badge"
          className="bg-destructive absolute -right-0.5 -bottom-0.5 size-2.5 rounded-full border border-black/30"
        />
      )}
    </span>
  );
}

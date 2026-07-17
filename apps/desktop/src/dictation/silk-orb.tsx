import { useEffect, useRef } from "react";

import type { DictationPhase } from "@hypr/plugin-dictation";

/**
 * "Silk" orb variant: a soft lavender ball with fine combed striations
 * turning over its surface, like brushed metal or a shell whorl. The palest
 * variant in the set, and like the other reference-derived orbs it owns a
 * fixed palette rather than inheriting `--primary`.
 *
 * The striations are lines of longitude on a turning sphere: each line's
 * projected width is `R * sin(phi)`, so they naturally bunch into a dense
 * comb near the silhouette and stretch out across the face - which is exactly
 * what the reference does, without needing a noise field.
 *
 * Phase mapping:
 * - idle: the comb turns slowly, low contrast.
 * - listening: the turn quickens and the lines sharpen with the voice.
 * - processing: a steady fast lathe.
 * - error: desaturates to ash and stops.
 *
 * The reference sits on white; the orb window is dark, so the ball fades to
 * transparent at its rim instead of into a white field.
 */

/** Keeps "listening" visibly alive during speech pauses. */
const LISTENING_FLOOR = 0.08;

/** Sampled from the reference. */
const CORE = "152 136 248";
const MID = "184 136 248";
const HIGHLIGHT = "185 130 255";
const WARM = "185 151 249";
const LINE = "232 224 255";
/** Blue-violet the ball falls to at its lower-right, giving it volume. */
const SHADOW = "108 96 208";
const ERROR_RGB = "150 145 150";

/** Longitude lines drawn around the ball. */
const LINE_COUNT = 22;

/** How much longitude the comb spans, radians. Less than 2pi leaves bare ball. */
const COMB_SPREAD = 1.9;

/** Drop longitudes whose projected width falls below this fraction of R. */
const EDGE_ON_CUTOFF = 0.16;

/** Radians of each longitude left undrawn at each pole. */
const POLE_GAP = 0.62;

/** Tilt of the comb's axis, radians - keeps the pole gaps off-vertical. */
const AXIS_TILT = -0.28;

/** Seconds per full revolution of the comb at rest (from the reference). */
const REVOLUTION = 15;

interface PhaseLook {
  /** Revolution speed multiplier. */
  spin: number;
  /** Line contrast floor. */
  lineAlpha: number;
  /** Constant intensity when not amplitude-driven. */
  intensityTarget: number;
}

const PHASE_LOOKS: Record<DictationPhase, PhaseLook> = {
  idle: { spin: 1, lineAlpha: 0.22, intensityTarget: 0 },
  listening: { spin: 2.4, lineAlpha: 0.34, intensityTarget: 0 },
  processing: { spin: 5, lineAlpha: 0.3, intensityTarget: 0.45 },
  error: { spin: 0, lineAlpha: 0.16, intensityTarget: 0 },
};

interface SilkState {
  time: number;
  intensity: number;
}

function clamp01(value: number): number {
  if (!Number.isFinite(value)) {
    return 0;
  }
  return Math.min(Math.max(value, 0), 1);
}

function rgba(rgb: string, alpha: number): string {
  return `rgba(${rgb.split(" ").join(", ")}, ${alpha.toFixed(3)})`;
}

function stepState(state: SilkState, phase: DictationPhase, amplitude: number) {
  const look = PHASE_LOOKS[phase];
  const target =
    phase === "listening"
      ? Math.max(LISTENING_FLOOR, clamp01(amplitude))
      : look.intensityTarget;

  state.intensity +=
    (target - state.intensity) * (target > state.intensity ? 0.18 : 0.05);

  state.time += (1 / 60) * look.spin * (1 + state.intensity * 0.6);
}

function drawFrame(
  ctx: CanvasRenderingContext2D,
  size: number,
  phase: DictationPhase,
  state: SilkState,
) {
  const look = PHASE_LOOKS[phase];
  const isError = phase === "error";
  const center = size / 2;
  const radius = center * 0.86;
  const { intensity, time } = state;

  ctx.clearRect(0, 0, size, size);

  const core = isError ? ERROR_RGB : CORE;
  const mid = isError ? ERROR_RGB : MID;

  ctx.save();
  ctx.beginPath();
  ctx.arc(center, center, radius, 0, Math.PI * 2);
  ctx.clip();

  // 1. Ball base: lit from the upper-left and falling to a deeper violet at
  // the lower-right, so it reads as a sphere rather than a flat lilac disc.
  // The rim dissolves instead of terminating - the reference has no hard edge.
  const ball = ctx.createRadialGradient(
    center - radius * 0.34,
    center - radius * 0.4,
    radius * 0.05,
    center,
    center,
    radius * 1.02,
  );
  ball.addColorStop(0, rgba(isError ? ERROR_RGB : HIGHLIGHT, 0.95));
  ball.addColorStop(0.4, rgba(mid, 0.92));
  ball.addColorStop(0.78, rgba(core, 0.85));
  ball.addColorStop(0.94, rgba(SHADOW, 0.6));
  ball.addColorStop(1, rgba(SHADOW, 0));
  ctx.fillStyle = ball;
  ctx.fillRect(0, 0, size, size);

  // 2. Warm subsurface bloom just left of center.
  if (!isError) {
    const bloom = ctx.createRadialGradient(
      center - radius * 0.18,
      center + radius * 0.05,
      0,
      center - radius * 0.18,
      center + radius * 0.05,
      radius * 0.5,
    );
    bloom.addColorStop(0, rgba(WARM, 0.5));
    bloom.addColorStop(1, rgba(WARM, 0));
    ctx.fillStyle = bloom;
    ctx.fillRect(0, 0, size, size);
  }

  // 3. The comb. Each line is a longitude at angle phi, its projected
  // half-width R*sin(phi) - but spacing them evenly reads as a wireframe
  // globe. The reference instead bunches them into a dense comb occupying
  // roughly a third of the ball that travels around it, fading out at both
  // ends, so most of the surface stays bare.
  const spin = (time / REVOLUTION) * Math.PI * 2;
  ctx.lineWidth = Math.max(0.3, radius * 0.01);
  ctx.lineCap = "round";
  for (let i = 0; i < LINE_COUNT; i += 1) {
    const t = i / (LINE_COUNT - 1);
    const phi = spin + (t - 0.5) * COMB_SPREAD;
    const rx = Math.abs(Math.sin(phi)) * radius;
    // A longitude turning edge-on collapses to a vertical line through the
    // centre; several of those stacked read as a hard seam, so drop them
    // before they degenerate and let the comb fade out instead.
    if (rx < radius * EDGE_ON_CUTOFF) {
      continue;
    }
    // Fade the comb at both ends so it has no hard start or stop, and again
    // as each line approaches edge-on.
    const ends = Math.sin(t * Math.PI);
    const edge = Math.min(1, (rx / radius - EDGE_ON_CUTOFF) / 0.25);
    // cos(phi) > 0 is the near face.
    const facing = Math.cos(phi);
    const alpha =
      (look.lineAlpha + intensity * 0.18) *
      ends *
      edge *
      (facing > 0 ? 1 : 0.28);
    if (alpha < 0.01) {
      continue;
    }
    ctx.strokeStyle = rgba(isError ? ERROR_RGB : LINE, alpha);
    // Every longitude passes through both poles, so drawing full ellipses
    // converges all of them onto two bright points and the ball reads as a
    // paper lantern. Draw each as two arcs that stop short of the poles,
    // leaving the comb open at top and bottom the way the reference is.
    // The axis is tilted so the gaps don't line up into a vertical stripe.
    ctx.beginPath();
    ctx.ellipse(
      center,
      center,
      rx,
      radius,
      AXIS_TILT,
      -Math.PI / 2 + POLE_GAP,
      Math.PI / 2 - POLE_GAP,
    );
    ctx.stroke();
    ctx.beginPath();
    ctx.ellipse(
      center,
      center,
      rx,
      radius,
      AXIS_TILT,
      Math.PI / 2 + POLE_GAP,
      (Math.PI * 3) / 2 - POLE_GAP,
    );
    ctx.stroke();
  }

  ctx.restore();

  // 4. A shaded terminator along the lower-right only, not a full outline:
  // a closed rim stroke reads as a drawn easter egg, whereas a partial arc
  // reads as the ball turning away from the light.
  const term = ctx.createLinearGradient(
    center - radius * 0.6,
    center - radius * 0.6,
    center + radius * 0.8,
    center + radius * 0.8,
  );
  term.addColorStop(0, rgba(isError ? ERROR_RGB : SHADOW, 0));
  term.addColorStop(0.55, rgba(isError ? ERROR_RGB : SHADOW, 0));
  term.addColorStop(1, rgba(isError ? ERROR_RGB : SHADOW, 0.5 + intensity * 0.2));
  ctx.strokeStyle = term;
  ctx.lineWidth = Math.max(0.6, radius * 0.05);
  ctx.beginPath();
  ctx.arc(center, center, radius - ctx.lineWidth / 2, 0, Math.PI * 2);
  ctx.stroke();
}

function prefersReducedMotion(): boolean {
  return (
    typeof window !== "undefined" &&
    typeof window.matchMedia === "function" &&
    window.matchMedia("(prefers-reduced-motion: reduce)").matches
  );
}

export function SilkOrb({
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
  const staticPhase = reducedMotion ? phase : null;

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

    const state: SilkState = { time: 3, intensity: 0 };

    if (reducedMotion) {
      state.intensity = phaseRef.current === "listening" ? 0.4 : 0;
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
  }, [size, reducedMotion, staticPhase]);

  return (
    <span
      data-testid="dictation-silk-orb"
      data-silk-phase={phase}
      className="relative inline-flex shrink-0"
      style={{ width: size, height: size }}
    >
      <canvas
        ref={canvasRef}
        aria-hidden
        style={{ width: size, height: size }}
      />
      {phase === "error" && (
        <span
          data-testid="dictation-silk-error-badge"
          className="bg-destructive absolute -right-0.5 -bottom-0.5 size-2.5 rounded-full border border-black/30"
        />
      )}
    </span>
  );
}

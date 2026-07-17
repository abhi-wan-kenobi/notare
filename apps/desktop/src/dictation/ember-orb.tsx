import { useEffect, useRef } from "react";

import type { DictationPhase } from "@hypr/plugin-dictation";

/**
 * "Ember" orb variant: a near-black glass ball with a magenta->orange caustic
 * band refracting through it. Like `bloom-orb.tsx` and `halo-orb.tsx`, Ember
 * owns a fixed palette - the hot magenta *is* the look, so it does not
 * inherit `--primary`.
 *
 * The band is the state channel: it rides low and dim at rest, and rises,
 * flattens and brightens with the voice.
 *
 * Phase mapping:
 * - idle: band low, dim, drifting slowly.
 * - listening: band rises/brightens with the live amplitude.
 * - processing: band sweeps the ball on the reference's ~4.9s cycle.
 * - error: band desaturates to ash and holds still.
 *
 * The reference's soft caustic falloff is faked with stacked translucent
 * strokes rather than `ctx.filter = "blur()"`, which is unreliable on
 * WebKitGTK (the Linux build's webview).
 */

/** Keeps "listening" visibly alive during speech pauses. */
const LISTENING_FLOOR = 0.08;

/** Sampled from the reference. */
const MAGENTA = "228 34 250";
const ORANGE = "240 122 32";
const RIM = "216 104 248";
const BODY_TOP = "24 8 24";
const BODY_BOTTOM = "10 4 12";
const ERROR_RGB = "150 145 140";

/** Seconds per band sweep at rest (measured off the reference). */
const SWEEP_PERIOD = 4.9;

interface PhaseLook {
  /** Band travel speed multiplier over SWEEP_PERIOD. */
  sweep: number;
  /** Band brightness floor. */
  bandAlpha: number;
  /** Constant intensity when not amplitude-driven. */
  intensityTarget: number;
  /** How many sparkle specks drift in the upper quadrant. */
  sparkles: number;
}

const PHASE_LOOKS: Record<DictationPhase, PhaseLook> = {
  idle: { sweep: 0.35, bandAlpha: 0.4, intensityTarget: 0, sparkles: 0 },
  listening: { sweep: 0.7, bandAlpha: 0.75, intensityTarget: 0, sparkles: 5 },
  processing: { sweep: 1.7, bandAlpha: 0.65, intensityTarget: 0.5, sparkles: 8 },
  error: { sweep: 0, bandAlpha: 0.35, intensityTarget: 0, sparkles: 0 },
};

interface EmberState {
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

function stepState(state: EmberState, phase: DictationPhase, amplitude: number) {
  const look = PHASE_LOOKS[phase];
  const target =
    phase === "listening"
      ? Math.max(LISTENING_FLOOR, clamp01(amplitude))
      : look.intensityTarget;

  state.intensity +=
    (target - state.intensity) * (target > state.intensity ? 0.2 : 0.045);

  state.time += (1 / 60) * look.sweep;
}

function drawFrame(
  ctx: CanvasRenderingContext2D,
  size: number,
  phase: DictationPhase,
  state: EmberState,
) {
  const look = PHASE_LOOKS[phase];
  const isError = phase === "error";
  const center = size / 2;
  const radius = center * 0.88;
  const { intensity, time } = state;

  ctx.clearRect(0, 0, size, size);

  const bright = look.bandAlpha + intensity * 0.35;
  const hot = isError ? ERROR_RGB : MAGENTA;
  const warm = isError ? ERROR_RGB : ORANGE;

  // 1. Outer atmospheric glow, hugging the rim.
  const glow = ctx.createRadialGradient(
    center,
    center,
    radius * 0.85,
    center,
    center,
    radius * 1.22,
  );
  glow.addColorStop(0, rgba(hot, 0.3 * bright));
  glow.addColorStop(1, rgba(hot, 0));
  ctx.fillStyle = glow;
  ctx.fillRect(0, 0, size, size);

  ctx.save();
  ctx.beginPath();
  ctx.arc(center, center, radius, 0, Math.PI * 2);
  ctx.clip();

  // 2. Sphere body: near-black, shaded darker toward the bottom-right.
  const body = ctx.createRadialGradient(
    center - radius * 0.3,
    center - radius * 0.35,
    0,
    center,
    center,
    radius * 1.15,
  );
  body.addColorStop(0, rgba(BODY_TOP, 1));
  body.addColorStop(1, rgba(BODY_BOTTOM, 1));
  ctx.fillStyle = body;
  ctx.fillRect(0, 0, size, size);

  // 3. The caustic band. It rides low at rest and rises as the voice comes
  // up; it also flattens out (the reference tilts ~-25deg -> ~0deg as it
  // sweeps upward), so tilt and height move together.
  const cycle = Math.sin((time / SWEEP_PERIOD) * Math.PI * 2);
  // y: +0.34r (low) at rest -> -0.06r (middle) when loud.
  const bandY = center + radius * (0.34 - intensity * 0.4 + cycle * 0.07);
  const tilt = (-0.42 + intensity * 0.38 + cycle * 0.08) * (isError ? 0 : 1);

  ctx.save();
  ctx.translate(center, bandY);
  ctx.rotate(tilt);

  const grad = ctx.createLinearGradient(-radius, 0, radius, 0);
  grad.addColorStop(0, rgba(hot, 0));
  grad.addColorStop(0.22, rgba(hot, 0.85));
  grad.addColorStop(0.55, rgba(hot, 1));
  grad.addColorStop(0.8, rgba(warm, 0.95));
  grad.addColorStop(1, rgba(warm, 0));

  // Stacked passes approximating a gaussian: widest and faintest first,
  // narrowing to a bright core. Few passes read as separate concentric
  // ribbons, so use enough that they blend into one soft caustic. Additive,
  // so the overlap is what produces the falloff.
  ctx.globalCompositeOperation = "lighter";
  ctx.lineCap = "round";
  ctx.strokeStyle = grad;
  const PASSES = 9;
  for (let i = 0; i < PASSES; i += 1) {
    // t: 0 at the widest pass -> 1 at the core.
    const t = i / (PASSES - 1);
    const width = 0.5 * (1 - t) ** 2 + 0.05;
    // Alpha climbs toward the core so the sum lands soft-edged, not banded.
    const alpha = 0.06 + 0.16 * t ** 2;
    // The band sits slightly lower than its core: a sharp top edge with the
    // falloff hanging beneath it, the way a real caustic refracts.
    const dy = radius * 0.09 * (1 - t);

    ctx.globalAlpha = alpha * bright;
    ctx.lineWidth = radius * width * (1 + intensity * 0.25);
    ctx.beginPath();
    ctx.moveTo(-radius * 1.05, dy + radius * 0.1);
    ctx.bezierCurveTo(
      -radius * 0.35,
      dy - radius * 0.16,
      radius * 0.35,
      dy + radius * 0.18,
      radius * 1.05,
      dy - radius * 0.08,
    );
    ctx.stroke();
  }
  ctx.globalAlpha = 1;
  ctx.globalCompositeOperation = "source-over";
  ctx.restore();

  // 4. Sparkle specks drifting in the upper-left quadrant.
  if (look.sparkles > 0 && !isError) {
    ctx.globalCompositeOperation = "lighter";
    for (let i = 0; i < look.sparkles; i += 1) {
      // Deterministic pseudo-random placement that drifts with time.
      const seed = i * 2.399;
      const sx =
        center + Math.sin(seed * 3.1 + time * 0.5) * radius * 0.45 - radius * 0.2;
      const sy =
        center + Math.cos(seed * 2.3 + time * 0.4) * radius * 0.4 - radius * 0.3;
      const twinkle = (Math.sin(time * 3 + seed * 5) + 1) / 2;
      ctx.fillStyle = rgba("255 235 250", 0.25 + twinkle * 0.5 * bright);
      ctx.beginPath();
      ctx.arc(sx, sy, Math.max(0.5, radius * 0.022), 0, Math.PI * 2);
      ctx.fill();
    }
    ctx.globalCompositeOperation = "source-over";
  }

  ctx.restore();

  // 5. Fresnel rim: brightest at lower-left and upper-right, which is what
  // sells it as glass rather than a flat disc.
  const rim = ctx.createLinearGradient(
    center - radius,
    center + radius,
    center + radius,
    center - radius,
  );
  const rimRgb = isError ? ERROR_RGB : RIM;
  rim.addColorStop(0, rgba(rimRgb, 0.85 * bright));
  rim.addColorStop(0.5, rgba(rimRgb, 0.18 * bright));
  rim.addColorStop(1, rgba(rimRgb, 0.7 * bright));
  ctx.strokeStyle = rim;
  ctx.lineWidth = Math.max(0.75, radius * 0.05);
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

export function EmberOrb({
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

    const state: EmberState = { time: 0, intensity: 0 };

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
      data-testid="dictation-ember-orb"
      data-ember-phase={phase}
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
          data-testid="dictation-ember-error-badge"
          className="bg-destructive absolute -right-0.5 -bottom-0.5 size-2.5 rounded-full border border-black/30"
        />
      )}
    </span>
  );
}

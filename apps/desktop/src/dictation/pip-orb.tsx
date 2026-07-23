import { useEffect, useRef } from "react";

import type { DictationPhase } from "@hypr/plugin-dictation";

/**
 * "Pip" orb variant: a squishy blue blob with a face. The outlier of the set
 * by design - where every other orb signals state through light, Pip signals
 * it through expression. It owns a fixed palette.
 *
 * Phase mapping (the eyes are the state channel):
 * - idle: sleepy half-lidded eyes, slow breathing, occasional blink.
 * - listening: wide eyes; the blob squashes and stretches with the voice.
 * - processing: eyes become a thinking "..." and the blob wobbles.
 * - error: flat dead eyes, a frown, desaturated.
 *
 * Canvas 2D. The blob body is a polar wobble (a circle with two travelling
 * sine harmonics) rather than a rigid disc, plus an offset ghost copy behind
 * it - the reference's "jelly with a shadow twin" read.
 */

/** Keeps "listening" visibly alive during speech pauses. */
const LISTENING_FLOOR = 0.1;

/** Sampled from the reference. */
const BODY = "77 88 245";
const GHOST = "16 20 90";
const FACE = "240 247 252";
const ERROR_BODY = "104 104 128";

interface PhaseLook {
  /** Blob wobble speed. */
  wobble: number;
  /** Wobble depth as a fraction of radius. */
  wobbleDepth: number;
  /** Breathing period, seconds. */
  breathePeriod: number;
  /** Constant intensity when not amplitude-driven. */
  intensityTarget: number;
}

const PHASE_LOOKS: Record<DictationPhase, PhaseLook> = {
  idle: {
    wobble: 0.5,
    wobbleDepth: 0.03,
    breathePeriod: 3.4,
    intensityTarget: 0,
  },
  listening: {
    wobble: 1.4,
    wobbleDepth: 0.05,
    breathePeriod: 1.6,
    intensityTarget: 0,
  },
  processing: {
    wobble: 2.6,
    wobbleDepth: 0.07,
    breathePeriod: 1,
    intensityTarget: 0.4,
  },
  // success is a transient end-of-session flourish (a variant-agnostic overlay
  // in orb.tsx); the body rests at the idle look.
  success: {
    wobble: 0.5,
    wobbleDepth: 0.03,
    breathePeriod: 3.4,
    intensityTarget: 0,
  },
  error: {
    wobble: 0,
    wobbleDepth: 0.015,
    breathePeriod: 5,
    intensityTarget: 0,
  },
};

/** Seconds between idle blinks. */
const BLINK_PERIOD = 4.2;
/** How long a blink takes. */
const BLINK_DURATION = 0.16;

interface PipState {
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

function stepState(state: PipState, phase: DictationPhase, amplitude: number) {
  const look = PHASE_LOOKS[phase];
  const target =
    phase === "listening"
      ? Math.max(LISTENING_FLOOR, clamp01(amplitude))
      : look.intensityTarget;

  state.intensity +=
    (target - state.intensity) * (target > state.intensity ? 0.25 : 0.06);

  state.time += 1 / 60;
}

/** Trace the wobbly blob body into the current path. */
function blobPath(
  ctx: CanvasRenderingContext2D,
  cx: number,
  cy: number,
  radius: number,
  time: number,
  look: PhaseLook,
  squash: number,
) {
  const STEPS = 48;
  ctx.beginPath();
  for (let i = 0; i <= STEPS; i += 1) {
    const a = (i / STEPS) * Math.PI * 2;
    // Two travelling harmonics keep the silhouette organic without noise.
    const wob =
      1 +
      look.wobbleDepth * Math.sin(a * 3 + time * look.wobble * 2.1) +
      look.wobbleDepth * 0.6 * Math.sin(a * 2 - time * look.wobble * 1.3);
    const r = radius * wob;
    const x = cx + Math.cos(a) * r * (1 / squash);
    const y = cy + Math.sin(a) * r * squash;
    if (i === 0) {
      ctx.moveTo(x, y);
    } else {
      ctx.lineTo(x, y);
    }
  }
  ctx.closePath();
}

/** How open the eyes are, 0 (shut) .. 1 (wide). */
function eyeOpen(phase: DictationPhase, time: number, intensity: number) {
  if (phase === "error") {
    return 0.12;
  }
  if (phase === "processing") {
    return 0.5;
  }

  const base = phase === "listening" ? 0.85 + intensity * 0.15 : 0.45;

  // Blink: a quick close/open on a fixed cadence.
  const t = time % BLINK_PERIOD;
  if (t < BLINK_DURATION) {
    const p = t / BLINK_DURATION;
    // 0 -> shut -> open again
    return base * Math.abs(Math.cos(p * Math.PI));
  }
  return base;
}

function drawFace(
  ctx: CanvasRenderingContext2D,
  cx: number,
  cy: number,
  radius: number,
  phase: DictationPhase,
  state: PipState,
) {
  const { time, intensity } = state;
  const eyeR = radius * 0.15;
  const eyeDx = radius * 0.32;
  const eyeY = cy - radius * 0.08;

  ctx.fillStyle = rgba(FACE, 0.95);
  ctx.strokeStyle = rgba(FACE, 0.95);
  ctx.lineCap = "round";

  if (phase === "processing") {
    // Thinking: three dots that pulse in sequence.
    const dotR = radius * 0.09;
    for (let i = 0; i < 3; i += 1) {
      const phase01 = (time * 1.6 - i * 0.22) % 1;
      const lift = Math.max(0, Math.sin(phase01 * Math.PI));
      ctx.globalAlpha = 0.4 + lift * 0.6;
      ctx.beginPath();
      ctx.arc(
        cx + (i - 1) * radius * 0.3,
        eyeY - lift * radius * 0.08,
        dotR,
        0,
        Math.PI * 2,
      );
      ctx.fill();
    }
    ctx.globalAlpha = 1;
    return;
  }

  const open = eyeOpen(phase, time, intensity);

  if (phase === "error") {
    // Flat dead eyes + a frown.
    ctx.lineWidth = Math.max(1, radius * 0.07);
    for (const dx of [-eyeDx, eyeDx]) {
      ctx.beginPath();
      ctx.moveTo(cx + dx - eyeR, eyeY);
      ctx.lineTo(cx + dx + eyeR, eyeY);
      ctx.stroke();
    }
    ctx.beginPath();
    ctx.arc(
      cx,
      cy + radius * 0.52,
      radius * 0.24,
      Math.PI * 1.2,
      Math.PI * 1.8,
    );
    ctx.stroke();
    return;
  }

  // Eyes: ellipses that squeeze shut as `open` goes to 0.
  for (const dx of [-eyeDx, eyeDx]) {
    ctx.beginPath();
    ctx.ellipse(
      cx + dx,
      eyeY,
      eyeR,
      Math.max(radius * 0.012, eyeR * open),
      0,
      0,
      Math.PI * 2,
    );
    ctx.fill();
  }

  // Mouth: a small smile that widens as the voice comes up.
  if (phase === "listening" && intensity > 0.25) {
    const w = radius * (0.16 + intensity * 0.16);
    const h = radius * (0.1 + intensity * 0.2);
    ctx.beginPath();
    ctx.ellipse(cx, cy + radius * 0.34, w, h, 0, 0, Math.PI);
    ctx.fill();
  } else {
    ctx.lineWidth = Math.max(1, radius * 0.06);
    ctx.beginPath();
    ctx.arc(
      cx,
      cy + radius * 0.26,
      radius * 0.18,
      Math.PI * 0.2,
      Math.PI * 0.8,
    );
    ctx.stroke();
  }
}

function drawFrame(
  ctx: CanvasRenderingContext2D,
  size: number,
  phase: DictationPhase,
  state: PipState,
) {
  const look = PHASE_LOOKS[phase];
  const isError = phase === "error";
  const center = size / 2;
  const { intensity, time } = state;

  ctx.clearRect(0, 0, size, size);

  const breathe =
    1 + 0.035 * Math.sin((time / look.breathePeriod) * Math.PI * 2);
  const radius = center * 0.66 * breathe * (1 + intensity * 0.1);
  // Loud voice squashes him wider - classic squash-and-stretch.
  const squash = 1 - intensity * 0.12;

  const body = isError ? ERROR_BODY : BODY;

  // Ghost twin behind, offset and slow - gives the jelly its depth.
  ctx.fillStyle = rgba(isError ? ERROR_BODY : GHOST, 0.85);
  blobPath(
    ctx,
    center + Math.sin(time * 0.6) * radius * 0.07,
    center + radius * 0.06,
    radius * 1.06,
    time * 0.7 + 2,
    look,
    squash,
  );
  ctx.fill();

  // Body.
  ctx.fillStyle = rgba(body, 1);
  blobPath(ctx, center, center, radius, time, look, squash);
  ctx.fill();

  // Specular sheen, upper-left.
  const sheen = ctx.createRadialGradient(
    center - radius * 0.4,
    center - radius * 0.45,
    0,
    center - radius * 0.4,
    center - radius * 0.45,
    radius * 0.9,
  );
  sheen.addColorStop(0, rgba(FACE, isError ? 0.06 : 0.22));
  sheen.addColorStop(1, rgba(FACE, 0));
  ctx.save();
  blobPath(ctx, center, center, radius, time, look, squash);
  ctx.clip();
  ctx.fillStyle = sheen;
  ctx.fillRect(0, 0, size, size);
  ctx.restore();

  drawFace(ctx, center, center, radius, phase, state);
}

function prefersReducedMotion(): boolean {
  return (
    typeof window !== "undefined" &&
    typeof window.matchMedia === "function" &&
    window.matchMedia("(prefers-reduced-motion: reduce)").matches
  );
}

export function PipOrb({
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

    const state: PipState = { time: 0.7, intensity: 0 };

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
      data-testid="dictation-pip-orb"
      data-pip-phase={phase}
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
          data-testid="dictation-pip-error-badge"
          className="bg-destructive absolute -right-0.5 -bottom-0.5 size-2.5 rounded-full border border-black/30"
        />
      )}
    </span>
  );
}

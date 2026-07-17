import { useEffect, useRef } from "react";

import type { DictationPhase } from "@hypr/plugin-dictation";

/**
 * "Bloom" orb variant: overlapping translucent petals that rearrange per
 * phase. Unlike the token-driven variants (`aurora-orb.tsx`, `ring-orb.tsx`),
 * Bloom owns a fixed warm palette - the amber/rose is the identity of the
 * look, so it deliberately does not inherit `--primary`.
 *
 * Phase mapping (petal count/arrangement is the state channel here, not just
 * brightness):
 * - idle: two small petals, barely drifting.
 * - listening: four petals fan out and swell with the live amplitude.
 * - processing: three petals tighten into a rosette and rotate together.
 * - error: petals desaturate to ash and stop.
 *
 * Canvas 2D, same renderer shape as `aurora-orb.tsx`: props mirrored into
 * refs so the rAF loop reads live values; `prefers-reduced-motion` draws a
 * single static frame.
 */

/** Keeps "listening" visibly alive during speech pauses. */
const LISTENING_FLOOR = 0.1;

/**
 * Base petal colors, sampled from the reference's opaque idle state. The
 * reference's apparent darker tones (browns, maroons) are not pigments - they
 * are these two colors *compositing over each other* at PETAL_ALPHA with plain
 * source-over blending, so only these two are real.
 */
const PETAL_COLORS = ["#F3A159", "#EF8983"];
const ERROR_COLOR = "#6B6560";

/**
 * Petals blend to the browns/maroons of the reference at ~60%. Idle is the
 * exception: with only two petals there is nothing to composite with, and the
 * reference shows them fully opaque.
 */
const PETAL_ALPHA = 0.6;

interface Petal {
  /** Angle around the center, radians. */
  angle: number;
  /** Distance from center as a fraction of the orb radius. */
  offset: number;
  /** Petal length/width as fractions of the orb radius. */
  length: number;
  width: number;
  /** Own tilt relative to its angle. */
  tilt: number;
  colorIndex: number;
}

/** An empty slot: a petal scaled to nothing, so bouquets can share indices. */
const NONE = { offset: 0, length: 0, width: 0, tilt: 0 } as const;

/**
 * Each phase gets its own bouquet, all sharing five slots so arrangements
 * morph slot-by-slot instead of popping. Geometry is transcribed from the
 * reference (bounding boxes measured in its 280x189 frame, normalized here):
 * idle reads ~54% of listening's width, and its 5th slot is unused.
 */
const PHASE_PETALS: Record<DictationPhase, Petal[]> = {
  // Two opaque blobs: a large orange with a smaller coral tucked at its
  // lower-left, overlapping ~30%. The reference holds this dead still.
  idle: [
    { angle: -0.6, offset: 0.1, length: 0.34, width: 0.32, tilt: 0.2, colorIndex: 0 },
    { angle: 2.5, offset: 0.16, length: 0.21, width: 0.2, tilt: -0.2, colorIndex: 1 },
    { angle: 1.1, ...NONE, colorIndex: 0 },
    { angle: 3.9, ...NONE, colorIndex: 1 },
    { angle: 5.4, ...NONE, colorIndex: 0 },
  ],
  // Loose 4-petal rosette: amber top, amber right, coral bottom, coral left.
  listening: [
    { angle: -1.4, offset: 0.26, length: 0.5, width: 0.44, tilt: 0.4, colorIndex: 0 },
    { angle: 0.3, offset: 0.28, length: 0.48, width: 0.42, tilt: -0.3, colorIndex: 0 },
    { angle: 1.7, offset: 0.26, length: 0.52, width: 0.46, tilt: 0.25, colorIndex: 1 },
    { angle: 3.4, offset: 0.27, length: 0.46, width: 0.4, tilt: -0.45, colorIndex: 1 },
    { angle: 5.1, ...NONE, colorIndex: 0 },
  ],
  // Same four petals pulled into a tighter, evenly-spaced rosette that spins.
  processing: [
    { angle: -0.8, offset: 0.19, length: 0.42, width: 0.38, tilt: 0.5, colorIndex: 0 },
    { angle: 0.8, offset: 0.19, length: 0.42, width: 0.38, tilt: 0.5, colorIndex: 1 },
    { angle: 2.4, offset: 0.19, length: 0.42, width: 0.38, tilt: 0.5, colorIndex: 0 },
    { angle: 4.0, offset: 0.19, length: 0.42, width: 0.38, tilt: 0.5, colorIndex: 1 },
    { angle: 5.6, ...NONE, colorIndex: 0 },
  ],
  error: [
    { angle: -0.6, offset: 0.1, length: 0.32, width: 0.3, tilt: 0.2, colorIndex: 0 },
    { angle: 2.5, offset: 0.16, length: 0.2, width: 0.19, tilt: -0.2, colorIndex: 1 },
    { angle: 1.1, ...NONE, colorIndex: 0 },
    { angle: 3.9, ...NONE, colorIndex: 1 },
    { angle: 5.4, ...NONE, colorIndex: 0 },
  ],
};

/**
 * The reference's "Speaking" bouquet: a 5th petal appears and the whole
 * rosette splays ~40% wider. Notare's orb has no speaking phase - it only
 * ever listens - but speaking *is* listening at high amplitude, so the
 * listening bouquet lerps toward this as you get louder. A loud voice blooms
 * the 5th petal; a quiet room settles back to four.
 */
const LISTENING_LOUD: Petal[] = [
  { angle: -1.5, offset: 0.36, length: 0.62, width: 0.5, tilt: 0.45, colorIndex: 0 },
  { angle: 0.2, offset: 0.38, length: 0.58, width: 0.48, tilt: -0.3, colorIndex: 0 },
  { angle: 1.6, offset: 0.36, length: 0.64, width: 0.52, tilt: 0.25, colorIndex: 1 },
  { angle: 3.3, offset: 0.37, length: 0.56, width: 0.46, tilt: -0.45, colorIndex: 1 },
  { angle: 4.9, offset: 0.34, length: 0.5, width: 0.42, tilt: 0.35, colorIndex: 1 },
];

/** Blend the quiet and loud listening bouquets by the amplitude envelope. */
function listeningBouquet(intensity: number, out: Petal[]) {
  const quiet = PHASE_PETALS.listening;
  for (let i = 0; i < out.length; i += 1) {
    const a = quiet[i];
    const b = LISTENING_LOUD[i];
    out[i].angle = a.angle + (b.angle - a.angle) * intensity;
    out[i].offset = a.offset + (b.offset - a.offset) * intensity;
    out[i].length = a.length + (b.length - a.length) * intensity;
    out[i].width = a.width + (b.width - a.width) * intensity;
    out[i].tilt = a.tilt + (b.tilt - a.tilt) * intensity;
    out[i].colorIndex = b.colorIndex;
  }
}

interface PhaseLook {
  /** Whole-rosette rotation, radians/sec. */
  spin: number;
  /** Per-petal independent wobble. */
  sway: number;
  /** Depth of the whole-bouquet scale oscillation (fraction of size). */
  breatheAmp: number;
  /** Seconds per breathe cycle. */
  breathePeriod: number;
  /** Constant intensity when not driven by amplitude. */
  intensityTarget: number;
  alpha: number;
}

/**
 * Timings transcribed from the reference: listening breathes +/-3% at ~0.95s;
 * processing pulses on a measured 1.333s period; idle is held still for
 * seconds at a time.
 */
const PHASE_LOOKS: Record<DictationPhase, PhaseLook> = {
  idle: {
    spin: 0.02,
    sway: 0.12,
    breatheAmp: 0.012,
    breathePeriod: 4,
    intensityTarget: 0,
    alpha: 1,
  },
  listening: {
    spin: 0,
    sway: 0.5,
    breatheAmp: 0.03,
    breathePeriod: 0.95,
    intensityTarget: 0,
    alpha: PETAL_ALPHA,
  },
  processing: {
    spin: 1.9,
    sway: 0.15,
    breatheAmp: 0,
    breathePeriod: 1.333,
    intensityTarget: 0.4,
    alpha: PETAL_ALPHA,
  },
  error: {
    spin: 0,
    sway: 0,
    breatheAmp: 0,
    breathePeriod: 1,
    intensityTarget: 0,
    alpha: 0.55,
  },
};

/** Seconds per processing contract/expand cycle (measured off the reference). */
const PROCESSING_PERIOD = 1.333;
/** Processing contracts to this fraction of its linear size at the trough. */
const PROCESSING_CONTRACTION = 0.72;

/**
 * The processing pulse is not a sine: the reference falls fast (~0.83s) and
 * springs back (~0.5s). Returns a 0..1 "how contracted" value.
 */
function processingPulse(time: number): number {
  const p = (time % PROCESSING_PERIOD) / PROCESSING_PERIOD;
  // Skew the phase so the fall occupies ~62% of the cycle and the spring ~38%.
  const shaped = p < 0.62 ? (p / 0.62) * 0.5 : 0.5 + ((p - 0.62) / 0.38) * 0.5;
  return (1 - Math.cos(shaped * Math.PI * 2)) / 2;
}

interface BloomState {
  /** Seconds since mount. Real time: the measured periods depend on it. */
  time: number;
  intensity: number;
  /** Current petal geometry, eased toward the active phase's bouquet. */
  petals: Petal[];
  /** Scratch for the phase's target bouquet, reused to avoid per-frame allocs. */
  goal: Petal[];
}

function clamp01(value: number): number {
  if (!Number.isFinite(value)) {
    return 0;
  }
  return Math.min(Math.max(value, 0), 1);
}

function makeState(phase: DictationPhase): BloomState {
  return {
    time: 0,
    intensity: 0,
    petals: PHASE_PETALS[phase].map((p) => ({ ...p })),
    goal: PHASE_PETALS[phase].map((p) => ({ ...p })),
  };
}

/**
 * The reference collapses Speaking->Idle over ~0.5s and blooms back over
 * ~0.5s; at 60fps this ease lands in the same neighbourhood.
 */
const MORPH_EASE = 0.08;

/** Ease the live petals toward the target bouquet so phases morph. */
function stepState(
  state: BloomState,
  phase: DictationPhase,
  amplitude: number,
) {
  const look = PHASE_LOOKS[phase];
  const target =
    phase === "listening"
      ? Math.max(LISTENING_FLOOR, clamp01(amplitude))
      : look.intensityTarget;

  // Fast attack, slow decay - matches the particle/aurora envelope.
  state.intensity +=
    (target - state.intensity) * (target > state.intensity ? 0.18 : 0.05);

  if (phase === "listening") {
    // Louder voice -> lerp toward the reference's 5-petal "speaking" splay.
    listeningBouquet(state.intensity, state.goal);
  } else {
    const preset = PHASE_PETALS[phase];
    for (let i = 0; i < state.goal.length; i += 1) {
      Object.assign(state.goal[i], preset[i]);
    }
  }

  for (let i = 0; i < state.petals.length; i += 1) {
    const p = state.petals[i];
    const g = state.goal[i];
    p.offset += (g.offset - p.offset) * MORPH_EASE;
    p.length += (g.length - p.length) * MORPH_EASE;
    p.width += (g.width - p.width) * MORPH_EASE;
    p.tilt += (g.tilt - p.tilt) * MORPH_EASE;
    // Angles are eased on the shortest arc so petals never spin the long way.
    let d = g.angle - p.angle;
    while (d > Math.PI) d -= Math.PI * 2;
    while (d < -Math.PI) d += Math.PI * 2;
    p.angle += d * MORPH_EASE;
    p.colorIndex = g.colorIndex;
  }

  state.time += 1 / 60;
}

function drawFrame(
  ctx: CanvasRenderingContext2D,
  size: number,
  phase: DictationPhase,
  state: BloomState,
) {
  const look = PHASE_LOOKS[phase];
  const center = size / 2;
  const radius = center * 0.92;
  const { intensity, time } = state;

  ctx.clearRect(0, 0, size, size);

  const spin = time * look.spin;

  // Whole-bouquet scale: the phase's own breathe, times the processing
  // contraction, times the amplitude swell. Only one of these is ever
  // meaningfully active per phase.
  const breathe =
    1 + look.breatheAmp * Math.sin((time / look.breathePeriod) * Math.PI * 2);
  const contraction =
    phase === "processing"
      ? 1 - (1 - PROCESSING_CONTRACTION) * processingPulse(time)
      : 1;
  const swell = breathe * contraction * (1 + intensity * 0.12);

  for (const petal of state.petals) {
    if (petal.length < 0.01 || petal.width < 0.01) {
      continue;
    }

    const sway =
      look.sway === 0
        ? 0
        : Math.sin(time * 1.3 + petal.angle * 2.1) * 0.16 * look.sway;
    const angle = petal.angle + spin + sway;
    // Offsets scale with the bouquet so it breathes as one body rather than
    // petals sliding against a fixed armature.
    const px = center + Math.cos(angle) * petal.offset * radius * swell;
    const py = center + Math.sin(angle) * petal.offset * radius * swell;

    ctx.save();
    ctx.translate(px, py);
    ctx.rotate(angle + petal.tilt + sway);
    ctx.globalAlpha = look.alpha;
    ctx.fillStyle =
      phase === "error" ? ERROR_COLOR : PETAL_COLORS[petal.colorIndex];
    ctx.beginPath();
    ctx.ellipse(
      0,
      0,
      petal.length * radius * swell,
      petal.width * radius * swell,
      0,
      0,
      Math.PI * 2,
    );
    ctx.fill();
    ctx.restore();
  }

  ctx.globalAlpha = 1;
}

function prefersReducedMotion(): boolean {
  return (
    typeof window !== "undefined" &&
    typeof window.matchMedia === "function" &&
    window.matchMedia("(prefers-reduced-motion: reduce)").matches
  );
}

export function BloomOrb({
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

    const state = makeState(phaseRef.current);

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
      data-testid="dictation-bloom-orb"
      data-bloom-phase={phase}
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
          data-testid="dictation-bloom-error-badge"
          className="bg-destructive absolute -right-0.5 -bottom-0.5 size-2.5 rounded-full border border-black/30"
        />
      )}
    </span>
  );
}

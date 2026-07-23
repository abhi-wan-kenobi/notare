import { useEffect, useRef } from "react";

import type { DictationPhase } from "@hypr/plugin-dictation";

/**
 * "Halo" orb variant: a neon rim with a ring of beads around its inner edge.
 * Like `bloom-orb.tsx`, Halo owns a fixed palette - the cyan/magenta sweep is
 * the identity of the look, so it does not inherit `--primary`.
 *
 * The reference's beads are an audio-spectrum ring: bead length varies with
 * angle and the tall arc migrates around the circle. That is literally a
 * radial EQ, so here the beads are driven by the live amplitude, and the
 * "orbiting envelope" is what the ring does when there is nothing to show.
 *
 * Phase mapping:
 * - idle: dim rim, beads flat, envelope creeping.
 * - listening: beads spike with the voice; rim brightens.
 * - processing: a dense arc of beads orbits at speed (spinner-like).
 * - error: rim desaturates to ash, beads flatten and stop.
 *
 * Canvas 2D, same renderer shape as `aurora-orb.tsx`.
 */

/** Keeps "listening" visibly alive during speech pauses. */
const LISTENING_FLOOR = 0.08;

/** Sampled from the reference. The rim sweeps cyan -> magenta -> cyan. */
const CYAN = "34 196 247";
const MAGENTA = "248 152 248";
const ERROR_RGB = "150 145 140";

/**
 * Beads sit on an annulus just inside the soft rim band. 44 is the most that
 * still leaves visible gaps at the 56px orb-window size.
 */
const BEAD_COUNT = 44;

interface PhaseLook {
  /** Revolutions/sec of the bead envelope around the ring. */
  envelopeSpin: number;
  /** How much of the circle the tall-bead arc covers, 0..1. */
  arcWidth: number;
  /** Rim brightness floor. */
  rimAlpha: number;
  /** Constant intensity when not driven by amplitude. */
  intensityTarget: number;
  /** Revolutions/sec of the inner dashed hairline. */
  hairlineSpin: number;
}

const PHASE_LOOKS: Record<DictationPhase, PhaseLook> = {
  idle: {
    envelopeSpin: 0.11,
    arcWidth: 0.42,
    rimAlpha: 0.3,
    intensityTarget: 0,
    hairlineSpin: 0.05,
  },
  listening: {
    envelopeSpin: 0.16,
    arcWidth: 0.5,
    rimAlpha: 0.6,
    intensityTarget: 0,
    hairlineSpin: 0.12,
  },
  processing: {
    envelopeSpin: 0.85,
    arcWidth: 0.3,
    rimAlpha: 0.5,
    intensityTarget: 0.6,
    hairlineSpin: 0.4,
  },
  // success is a transient end-of-session flourish (a variant-agnostic overlay
  // in orb.tsx); the body rests at the idle look.
  success: {
    envelopeSpin: 0.11,
    arcWidth: 0.42,
    rimAlpha: 0.3,
    intensityTarget: 0,
    hairlineSpin: 0.05,
  },
  error: {
    envelopeSpin: 0,
    arcWidth: 0.42,
    rimAlpha: 0.45,
    intensityTarget: 0,
    hairlineSpin: 0,
  },
};

interface HaloState {
  time: number;
  intensity: number;
  /** Per-bead heights, smoothed frame to frame so the ring never strobes. */
  beads: number[];
}

function clamp01(value: number): number {
  if (!Number.isFinite(value)) {
    return 0;
  }
  return Math.min(Math.max(value, 0), 1);
}

function makeState(): HaloState {
  return { time: 0, intensity: 0, beads: new Array(BEAD_COUNT).fill(0) };
}

function rgba(rgb: string, alpha: number): string {
  return `rgba(${rgb.split(" ").join(", ")}, ${alpha.toFixed(3)})`;
}

/** Blend two "r g b" triples. */
function mixRgb(a: string, b: string, t: number): string {
  const pa = a.split(" ").map(Number);
  const pb = b.split(" ").map(Number);
  return pa.map((v, i) => Math.round(v + (pb[i] - v) * t)).join(" ");
}

function stepState(state: HaloState, phase: DictationPhase, amplitude: number) {
  const look = PHASE_LOOKS[phase];
  const target =
    phase === "listening"
      ? Math.max(LISTENING_FLOOR, clamp01(amplitude))
      : look.intensityTarget;

  state.intensity +=
    (target - state.intensity) * (target > state.intensity ? 0.2 : 0.05);

  const envelopeCenter = state.time * look.envelopeSpin;

  for (let i = 0; i < BEAD_COUNT; i += 1) {
    const at = i / BEAD_COUNT;
    // Angular distance from the envelope's center, wrapped to 0..0.5.
    let d = Math.abs(((at - envelopeCenter) % 1) + 1) % 1;
    if (d > 0.5) {
      d = 1 - d;
    }
    // Cosine falloff across the arc: tall in the middle, flat outside.
    const inArc = clamp01(1 - d / (look.arcWidth * 0.5));
    const envelope = (1 - Math.cos(inArc * Math.PI)) / 2;

    // Per-bead jitter keeps the arc from reading as a smooth lump. Two
    // incommensurate sines so it never visibly repeats.
    const jitter =
      0.5 +
      0.3 * Math.sin(state.time * 5.2 + i * 1.7) +
      0.2 * Math.sin(state.time * 3.1 + i * 0.6);

    const target = envelope * (0.25 + state.intensity * 0.75) * jitter;
    // Fast attack, slow release - reads like a real meter.
    const bead = state.beads[i];
    state.beads[i] += (target - bead) * (target > bead ? 0.45 : 0.12);
  }

  state.time += 1 / 60;
}

/**
 * The rim band's stroke: a conic gradient with two cyan nodes and two magenta
 * nodes, rotating so the hues trade sides. `createConicGradient` is missing on
 * older WebKitGTK, so fall back to a flat blend rather than throwing - the
 * ring keeps its shape and only loses the sweep.
 */
function bandStroke(
  ctx: CanvasRenderingContext2D,
  center: number,
  time: number,
  isError: boolean,
  bright: number,
): string | CanvasGradient {
  // The band is the orb's main light source, so it runs near-opaque; the
  // rest of the look is calibrated against it being vivid rather than tinted.
  const alpha = Math.min(1, 0.62 + bright * 0.38);
  const flat = rgba(isError ? ERROR_RGB : mixRgb(CYAN, MAGENTA, 0.5), alpha);
  if (isError || typeof ctx.createConicGradient !== "function") {
    return flat;
  }

  const g = ctx.createConicGradient(-time * 0.55, center, center);
  // Four stops so cyan and magenta each appear twice around the ring, with
  // a lifted midpoint between them - a straight cyan/magenta blend passes
  // through a muddy grey-violet at the halfway mark.
  const NODES = 4;
  for (let i = 0; i <= NODES * 2; i += 1) {
    const at = i / (NODES * 2);
    const even = i % 2 === 0;
    const rgb = even
      ? i % 4 === 0
        ? CYAN
        : MAGENTA
      : mixRgb(CYAN, MAGENTA, 0.5);
    g.addColorStop(at, rgba(rgb, even ? alpha : alpha * 0.82));
  }
  return g;
}

function drawFrame(
  ctx: CanvasRenderingContext2D,
  size: number,
  phase: DictationPhase,
  state: HaloState,
) {
  const look = PHASE_LOOKS[phase];
  const isError = phase === "error";
  const center = size / 2;
  const outer = center * 0.9;
  const { intensity, time } = state;

  ctx.clearRect(0, 0, size, size);

  const bright = look.rimAlpha + intensity * 0.4;

  // 1. Outer bloom. A wide, very soft halo - the reference's glow reaches
  // ~1.35x the ring radius before it hits black.
  const bloom = ctx.createRadialGradient(
    center,
    center,
    outer * 0.55,
    center,
    center,
    outer * 1.35,
  );
  const bloomRgb = isError ? ERROR_RGB : mixRgb(CYAN, MAGENTA, 0.5);
  bloom.addColorStop(0, rgba(bloomRgb, 0.4 * bright));
  bloom.addColorStop(0.45, rgba(bloomRgb, 0.16 * bright));
  bloom.addColorStop(1, rgba(bloomRgb, 0));
  ctx.fillStyle = bloom;
  ctx.fillRect(0, 0, size, size);

  // 2. Soft rim band. A conic gradient sweeps cyan and magenta around the
  // ring in one smooth pass - stitching it from short arcs leaves visible
  // round-capped lumps, which is the one thing this look cannot have.
  const bandWidth = Math.max(1.5, outer * 0.13);
  ctx.lineWidth = bandWidth;
  ctx.strokeStyle = bandStroke(ctx, center, time, isError, bright);
  ctx.beginPath();
  ctx.arc(center, center, outer * 0.88, 0, Math.PI * 2);
  ctx.stroke();

  // 3. The bead ring: the state channel. Each bead is a radial tick hanging
  // *inward* from just inside the band - growing them outward instead makes
  // them cross the band and the layering turns to mush.
  const beadTop = outer * 0.79;
  const beadMax = outer * 0.26;
  ctx.lineCap = "round";
  ctx.lineWidth = Math.max(0.7, outer * 0.038);
  for (let i = 0; i < BEAD_COUNT; i += 1) {
    const level = state.beads[i];
    if (level < 0.02) {
      continue;
    }
    const a = (i / BEAD_COUNT) * Math.PI * 2;
    const cos = Math.cos(a);
    const sin = Math.sin(a);
    // A stub always shows, so the ring reads as a meter even at rest.
    const r1 = beadTop;
    const r0 = beadTop - beadMax * level - outer * 0.02;
    const rgb = isError ? ERROR_RGB : mixRgb(CYAN, MAGENTA, (sin + 1) / 2);
    ctx.strokeStyle = rgba(rgb, 0.5 + level * 0.5);
    ctx.beginPath();
    ctx.moveTo(center + cos * r0, center + sin * r0);
    ctx.lineTo(center + cos * r1, center + sin * r1);
    ctx.stroke();
  }

  // 4. Inner dashed hairline at ~0.86x the bead base - the fine dotted
  // outline that gives the reference its precision feel. setLineDash keeps
  // the dashes even and hair-thin; stitching arcs by hand reads chunky.
  if (!isError) {
    // Sits inside the beads' deepest reach (0.79 - 0.26 - 0.02) so the two
    // rings never collide.
    const hairR = outer * 0.44;
    const circumference = 2 * Math.PI * hairR;
    // ~64 dashes around, each dash a third of its slot.
    const slot = circumference / 64;
    ctx.save();
    ctx.lineCap = "butt";
    ctx.setLineDash([slot * 0.34, slot * 0.66]);
    ctx.lineDashOffset = -time * look.hairlineSpin * circumference;
    ctx.lineWidth = Math.max(0.4, outer * 0.014);
    ctx.strokeStyle = rgba(MAGENTA, 0.3 + intensity * 0.28);
    ctx.beginPath();
    ctx.arc(center, center, hairR, 0, Math.PI * 2);
    ctx.stroke();
    ctx.restore();
  }
}

function prefersReducedMotion(): boolean {
  return (
    typeof window !== "undefined" &&
    typeof window.matchMedia === "function" &&
    window.matchMedia("(prefers-reduced-motion: reduce)").matches
  );
}

export function HaloOrb({
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

    const state = makeState();

    if (reducedMotion) {
      // Settle the beads into a representative static ring, then draw once.
      state.intensity = phaseRef.current === "listening" ? 0.4 : 0;
      for (let i = 0; i < 90; i += 1) {
        stepState(state, phaseRef.current, amplitudeRef.current);
      }
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
      data-testid="dictation-halo-orb"
      data-halo-phase={phase}
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
          data-testid="dictation-halo-error-badge"
          className="bg-destructive absolute -right-0.5 -bottom-0.5 size-2.5 rounded-full border border-black/30"
        />
      )}
    </span>
  );
}

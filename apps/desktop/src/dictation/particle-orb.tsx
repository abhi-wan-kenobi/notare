import { useEffect, useRef } from "react";

import type { DictationPhase } from "@hypr/plugin-dictation";

/**
 * "Particles" orb variant: a voice-reactive particle sphere, recreated from
 * the Voice Orb reference (voice-orb.netlify.app, particle style) at
 * dictation-orb scale. The reference renders 150k WebGL points with
 * curl-noise flow; at 40px a 2D canvas with a few hundred points and a cheap
 * sin-field turbulence is visually equivalent and dependency-free.
 *
 * Faithful to the reference:
 * - sphere distribution: 60% in a dense shell (r 0.95-1.10), 30% inner fill
 *   (r 0.40-0.95), 10% outer halo (r 1.10-1.60);
 * - palette: indigo #6366F1 -> purple #A855F7, shifting toward pink #EC4899
 *   as the voice intensity rises; bright additive cores with soft halos;
 * - smoothed intensity envelope (fast attack 0.15, slow decay 0.04) driving
 *   particle agitation, brightness, size and rotation speed;
 * - flow time that accelerates with intensity so the cloud "breathes harder"
 *   with voice.
 *
 * Phase mapping: idle = slow drift; listening = amplitude-reactive (the
 * reference's "speaking"); processing = faster orbital churn (its
 * "thinking"); error = dim, desaturated, still.
 *
 * `prefers-reduced-motion` renders a static sphere (single frame per phase,
 * no animation loop).
 */

export const PARTICLE_ORB_PARTICLE_COUNT = 340;

/** Orb diameter (px) the reference tuning above was made at. */
export const PARTICLE_ORB_BASE_SIZE = 40;

/**
 * Particle count for an orb of `size` px: grows linearly with the diameter
 * (340 at 40px -> 510 at 60px). Together with the sqrt sprite damping in
 * `drawFrame` this keeps total glow ~ count x radius^2 proportional to the
 * orb area, so density looks the same at every size.
 */
export function particleCountForSize(size: number): number {
  return Math.max(
    1,
    Math.round((PARTICLE_ORB_PARTICLE_COUNT * size) / PARTICLE_ORB_BASE_SIZE),
  );
}

/** Reference palette (indigo-500, purple-500, pink-500). */
const COLOR_1: Rgb = [99, 102, 241];
const COLOR_2: Rgb = [168, 85, 247];
const COLOR_3: Rgb = [236, 72, 153];
/** Error look: dim desaturated slate. */
const COLOR_ERROR: Rgb = [148, 155, 175];

/** Perspective camera: distance and focal length (reference camera z=3). */
const CAMERA_DISTANCE = 3;
/** Sphere radius in px = (size/2) / MAX_EXTENT so the halo shell fits. */
const MAX_EXTENT = 1.75;

type Rgb = readonly [number, number, number];

export interface Particle {
  x: number;
  y: number;
  z: number;
  seed: number;
  size: number;
}

/**
 * Radius distribution of the reference: dense shell / inner fill / halo.
 * Exported (with an injectable RNG) for tests.
 */
export function createParticles(
  count: number,
  random: () => number = Math.random,
): Particle[] {
  const particles: Particle[] = [];

  for (let i = 0; i < count; i++) {
    const cosTheta = 2 * random() - 1;
    const phi = random() * Math.PI * 2;
    const sinTheta = Math.sqrt(1 - cosTheta * cosTheta);

    const bucket = random();
    const r =
      bucket < 0.6
        ? 0.95 + 0.15 * random()
        : bucket < 0.9
          ? 0.4 + 0.55 * random()
          : 1.1 + 0.5 * random();

    particles.push({
      x: r * sinTheta * Math.cos(phi),
      y: r * sinTheta * Math.sin(phi),
      z: r * cosTheta,
      seed: random(),
      size: 0.3 + 0.7 * random(),
    });
  }

  return particles;
}

interface PhaseLook {
  rotY: number;
  rotX: number;
  turbulence: number;
  brightness: number;
  /** Constant intensity target (listening uses the live amplitude instead). */
  intensityTarget: number;
}

/** Per-frame motion constants, scaled from the reference's per-state values. */
const PHASE_LOOKS: Record<DictationPhase, PhaseLook> = {
  idle: {
    rotY: 0.0015,
    rotX: 0,
    turbulence: 0.1,
    brightness: 0.85,
    intensityTarget: 0,
  },
  listening: {
    rotY: 0.003,
    rotX: 0,
    turbulence: 0.1,
    brightness: 1,
    intensityTarget: 0,
  },
  processing: {
    rotY: 0.012,
    rotX: 0.004,
    turbulence: 0.14,
    brightness: 0.95,
    intensityTarget: 0.3,
  },
  // success is a transient end-of-session flourish (a variant-agnostic overlay
  // in orb.tsx); the body rests at the idle look.
  success: {
    rotY: 0.0015,
    rotX: 0,
    turbulence: 0.1,
    brightness: 0.85,
    intensityTarget: 0,
  },
  error: {
    rotY: 0,
    rotX: 0,
    turbulence: 0,
    brightness: 0.45,
    intensityTarget: 0,
  },
};

function mixRgb(a: Rgb, b: Rgb, t: number): Rgb {
  return [
    Math.round(a[0] + (b[0] - a[0]) * t),
    Math.round(a[1] + (b[1] - a[1]) * t),
    Math.round(a[2] + (b[2] - a[2]) * t),
  ];
}

/**
 * Pre-rendered point sprite: bright near-white core + soft colored halo
 * (bakes the reference fragment shader's core/halo falloff).
 */
function createSprite(color: Rgb, spriteSize: number): HTMLCanvasElement {
  const sprite = document.createElement("canvas");
  sprite.width = spriteSize;
  sprite.height = spriteSize;

  const ctx = sprite.getContext("2d");
  if (!ctx) {
    return sprite;
  }

  const half = spriteSize / 2;
  const core = mixRgb(color, [255, 255, 255], 0.65);
  const gradient = ctx.createRadialGradient(half, half, 0, half, half, half);
  gradient.addColorStop(0, `rgba(${core[0]}, ${core[1]}, ${core[2]}, 1)`);
  gradient.addColorStop(
    0.15,
    `rgba(${color[0]}, ${color[1]}, ${color[2]}, 0.55)`,
  );
  gradient.addColorStop(
    0.5,
    `rgba(${color[0]}, ${color[1]}, ${color[2]}, 0.16)`,
  );
  gradient.addColorStop(1, `rgba(${color[0]}, ${color[1]}, ${color[2]}, 0)`);

  ctx.fillStyle = gradient;
  ctx.fillRect(0, 0, spriteSize, spriteSize);
  return sprite;
}

const SPRITE_STEPS = 8;
const SPRITE_SIZE = 32;

interface SpriteSet {
  /** Indigo -> purple ramp, indexed by particle seed. */
  base: HTMLCanvasElement[];
  /** Purple -> pink ramp for high voice intensity. */
  hot: HTMLCanvasElement[];
  error: HTMLCanvasElement;
}

function createSprites(): SpriteSet {
  const base: HTMLCanvasElement[] = [];
  const hot: HTMLCanvasElement[] = [];
  for (let i = 0; i < SPRITE_STEPS; i++) {
    const t = i / (SPRITE_STEPS - 1);
    base.push(createSprite(mixRgb(COLOR_1, COLOR_2, t), SPRITE_SIZE));
    hot.push(createSprite(mixRgb(COLOR_2, COLOR_3, t), SPRITE_SIZE));
  }
  return { base, hot, error: createSprite(COLOR_ERROR, SPRITE_SIZE) };
}

interface RendererState {
  time: number;
  flowTime: number;
  rotY: number;
  rotX: number;
  intensity: number;
}

function clamp01(value: number): number {
  return Math.min(Math.max(value, 0), 1);
}

function drawFrame(
  ctx: CanvasRenderingContext2D,
  particles: Particle[],
  sprites: SpriteSet,
  size: number,
  phase: DictationPhase,
  state: RendererState,
) {
  const look = PHASE_LOOKS[phase];
  const { intensity } = state;

  ctx.clearRect(0, 0, size, size);
  ctx.globalCompositeOperation = "lighter";

  const center = size / 2;
  const unit = center / MAX_EXTENT;
  // Sprites grow sub-linearly (sqrt) with the diameter while the particle
  // count grows linearly (`particleCountForSize`), keeping the perceived
  // density constant instead of turning bigger orbs into fewer, fatter blobs.
  const spriteScale = Math.sqrt(PARTICLE_ORB_BASE_SIZE / size);
  const cosY = Math.cos(state.rotY);
  const sinY = Math.sin(state.rotY);
  const cosX = Math.cos(state.rotX);
  const sinX = Math.sin(state.rotX);

  const turbulence = look.turbulence * (1 + intensity * 1.4);
  const ft = state.flowTime;

  for (const p of particles) {
    // Cheap sin-field turbulence standing in for the reference's curl noise.
    const px =
      p.x + turbulence * Math.sin(ft * 0.9 + p.y * 2.0 + p.seed * 6.28);
    const py = p.y + turbulence * Math.sin(ft * 1.1 + p.z * 1.8 + p.seed * 4.7);
    const pz = p.z + turbulence * Math.sin(ft * 0.8 + p.x * 2.2 + p.seed * 3.1);

    // Rotate around Y, then X.
    const xr = px * cosY + pz * sinY;
    const zr1 = -px * sinY + pz * cosY;
    const yr = py * cosX - zr1 * sinX;
    const zr = py * sinX + zr1 * cosX;

    // Perspective projection (reference: point size ~ 1 / (cameraZ - z)).
    const w = CAMERA_DISTANCE / (CAMERA_DISTANCE - zr * 0.9);
    const sx = center + xr * unit * w;
    const sy = center + yr * unit * w;

    const radius =
      p.size * (0.055 + intensity * 0.025) * unit * w * spriteScale;
    const spriteRadius = radius * 3.2;

    const alpha =
      (0.2 + p.seed * 0.3) * look.brightness * (0.8 + intensity * 0.9);
    ctx.globalAlpha = clamp01(alpha);

    const sprite =
      phase === "error"
        ? sprites.error
        : intensity > 0.5 && p.seed < (intensity - 0.5) * 2
          ? sprites.hot[Math.floor(p.seed * SPRITE_STEPS) % SPRITE_STEPS]
          : sprites.base[Math.floor(p.seed * SPRITE_STEPS) % SPRITE_STEPS];

    ctx.drawImage(
      sprite,
      sx - spriteRadius,
      sy - spriteRadius,
      spriteRadius * 2,
      spriteRadius * 2,
    );
  }

  ctx.globalAlpha = 1;
  ctx.globalCompositeOperation = "source-over";
}

function stepState(
  state: RendererState,
  phase: DictationPhase,
  amplitude: number,
) {
  const look = PHASE_LOOKS[phase];
  const target =
    phase === "listening" ? clamp01(amplitude) : look.intensityTarget;

  // Reference envelope: fast attack, slow decay.
  state.intensity +=
    (target - state.intensity) * (target > state.intensity ? 0.15 : 0.04);

  state.time += 0.016;
  state.flowTime += 0.016 * (1 + 3.5 * state.intensity);
  state.rotY += look.rotY * (1 + state.intensity * 1.5);
  state.rotX += look.rotX;
}

function prefersReducedMotion(): boolean {
  return (
    typeof window !== "undefined" &&
    typeof window.matchMedia === "function" &&
    window.matchMedia("(prefers-reduced-motion: reduce)").matches
  );
}

export function ParticleOrb({
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
  // With motion reduced there is no animation loop to pick phase changes up,
  // so the phase becomes an effect dependency and each change redraws the
  // static frame. The animated path reads the phase from the ref instead.
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

    const particles = createParticles(particleCountForSize(size));
    const sprites = createSprites();
    const state: RendererState = {
      time: 0,
      flowTime: 0,
      rotY: 0.6,
      rotX: 0,
      intensity: 0,
    };

    if (reducedMotion) {
      // Static sphere: a single, motionless frame for the current phase.
      state.intensity = phaseRef.current === "listening" ? 0.4 : 0;
      drawFrame(ctx, particles, sprites, size, phaseRef.current, state);
      return;
    }

    let raf = 0;
    const tick = () => {
      stepState(state, phaseRef.current, amplitudeRef.current);
      drawFrame(ctx, particles, sprites, size, phaseRef.current, state);
      raf = requestAnimationFrame(tick);
    };
    raf = requestAnimationFrame(tick);

    return () => {
      cancelAnimationFrame(raf);
    };
  }, [size, reducedMotion, staticPhase]);

  return (
    <span
      className="relative inline-flex shrink-0"
      style={{ width: size, height: size }}
    >
      <canvas
        ref={canvasRef}
        data-testid="dictation-particle-orb"
        aria-hidden
        style={{ width: size, height: size }}
      />
      {phase === "error" && (
        <span
          data-testid="dictation-particle-error-badge"
          className="bg-destructive absolute -right-0.5 -bottom-0.5 size-2.5 rounded-full border border-black/30"
        />
      )}
    </span>
  );
}

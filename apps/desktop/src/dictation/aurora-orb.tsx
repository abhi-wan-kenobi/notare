import { useEffect, useRef } from "react";

import type { DictationPhase } from "@hypr/plugin-dictation";

/**
 * "Aurora" orb variant: soft layered gradient blobs drifting inside a round
 * clip, brightening and churning with the voice. Built like the particle
 * sphere (`particle-orb.tsx`): a small 2D-canvas renderer driven by
 * phase/amplitude with a smoothed intensity envelope.
 *
 * Colors come from the design tokens (`--primary`, `--accent-glow`,
 * `--accent-glow-end` in packages/ui/src/styles/globals.css), resolved from
 * the computed style at mount so the orb follows the token set of whatever
 * surface hosts it; the literals below are only the documented dark-theme
 * token values as fallbacks for environments without the stylesheet (tests).
 *
 * Phase mapping: idle = slow dim drift; listening = brightness/turbulence
 * track the live amplitude; processing = constant medium churn; error =
 * still, desaturated. `prefers-reduced-motion` renders one static frame.
 */

/** Dark-theme token values (globals.css) used when the CSS vars are absent. */
const TOKEN_FALLBACKS: Record<string, string> = {
  "--primary": "235 100% 65%",
  "--accent-glow": "190 100% 72%",
  "--accent-glow-end": "255 92% 76%",
};

function resolveToken(name: string): string {
  if (typeof window !== "undefined" && typeof getComputedStyle === "function") {
    const value = getComputedStyle(document.documentElement)
      .getPropertyValue(name)
      .trim();
    if (value) {
      return value;
    }
  }
  return TOKEN_FALLBACKS[name] ?? "0 0% 100%";
}

function hsl(token: string, alpha: number): string {
  return `hsl(${token} / ${alpha.toFixed(3)})`;
}

interface Blob {
  /** Orbit radius as a fraction of the orb radius. */
  orbit: number;
  /** Base angle + per-blob angular speed. */
  angle: number;
  speed: number;
  /** Blob radius as a fraction of the orb radius. */
  radius: number;
  /** Which color this blob draws with. */
  colorIndex: number;
}

const BLOBS: Blob[] = [
  { orbit: 0.28, angle: 0.4, speed: 0.35, radius: 0.62, colorIndex: 0 },
  { orbit: 0.34, angle: 2.4, speed: -0.27, radius: 0.5, colorIndex: 1 },
  { orbit: 0.3, angle: 4.6, speed: 0.21, radius: 0.55, colorIndex: 2 },
];

interface PhaseLook {
  /** Base drift speed multiplier. */
  drift: number;
  /** Base brightness. */
  brightness: number;
  /** Constant intensity target (listening uses the live amplitude). */
  intensityTarget: number;
}

const PHASE_LOOKS: Record<DictationPhase, PhaseLook> = {
  idle: { drift: 0.35, brightness: 0.4, intensityTarget: 0 },
  listening: { drift: 1, brightness: 0.75, intensityTarget: 0 },
  processing: { drift: 1.6, brightness: 0.6, intensityTarget: 0.35 },
  error: { drift: 0, brightness: 0.3, intensityTarget: 0 },
};

interface AuroraState {
  time: number;
  intensity: number;
}

function clamp01(value: number): number {
  return Math.min(Math.max(value, 0), 1);
}

function drawFrame(
  ctx: CanvasRenderingContext2D,
  size: number,
  phase: DictationPhase,
  state: AuroraState,
  colors: string[],
) {
  const look = PHASE_LOOKS[phase];
  const center = size / 2;
  const orbRadius = center * 0.94;
  const { intensity } = state;

  ctx.clearRect(0, 0, size, size);

  ctx.save();
  ctx.beginPath();
  ctx.arc(center, center, orbRadius, 0, Math.PI * 2);
  ctx.clip();

  // Graphite base so the blobs sit on a disc rather than floating shards.
  const base = ctx.createRadialGradient(
    center,
    center,
    0,
    center,
    center,
    orbRadius,
  );
  const baseAlpha = phase === "error" ? 0.5 : 0.65;
  base.addColorStop(0, `rgba(18, 21, 29, ${baseAlpha})`);
  base.addColorStop(1, `rgba(11, 13, 18, ${baseAlpha})`);
  ctx.fillStyle = base;
  ctx.fillRect(0, 0, size, size);

  ctx.globalCompositeOperation = "lighter";

  const brightness = look.brightness * (1 + intensity * 1.1);
  // Turbulence: intensity widens the orbits and swells the blobs.
  const swell = 1 + intensity * 0.35;

  for (const blob of BLOBS) {
    const angle = blob.angle + state.time * blob.speed;
    const wobble =
      1 + 0.18 * Math.sin(state.time * 0.7 + blob.angle * 3) * (1 + intensity);
    const bx = center + Math.cos(angle) * blob.orbit * orbRadius * wobble;
    const by = center + Math.sin(angle) * blob.orbit * orbRadius * wobble;
    const br = blob.radius * orbRadius * swell;

    const color =
      phase === "error" ? "220 10% 60%" : colors[blob.colorIndex % colors.length];
    const gradient = ctx.createRadialGradient(bx, by, 0, bx, by, br);
    gradient.addColorStop(0, hsl(color, clamp01(0.5 * brightness)));
    gradient.addColorStop(0.55, hsl(color, clamp01(0.18 * brightness)));
    gradient.addColorStop(1, hsl(color, 0));

    ctx.fillStyle = gradient;
    ctx.beginPath();
    ctx.arc(bx, by, br, 0, Math.PI * 2);
    ctx.fill();
  }

  ctx.globalCompositeOperation = "source-over";
  ctx.restore();

  // Hairline rim so the disc reads at 56px on any background.
  ctx.beginPath();
  ctx.arc(center, center, orbRadius - 0.5, 0, Math.PI * 2);
  ctx.strokeStyle =
    phase === "error"
      ? hsl("0 63% 45%", 0.7)
      : hsl(colors[0], 0.25 + intensity * 0.35);
  ctx.lineWidth = 1;
  ctx.stroke();
}

function stepState(
  state: AuroraState,
  phase: DictationPhase,
  amplitude: number,
) {
  const look = PHASE_LOOKS[phase];
  const target =
    phase === "listening" ? clamp01(amplitude) : look.intensityTarget;

  // Same envelope feel as the particle orb: fast attack, slow decay.
  state.intensity +=
    (target - state.intensity) * (target > state.intensity ? 0.15 : 0.04);

  state.time += 0.016 * look.drift * (1 + state.intensity * 1.6);
}

function prefersReducedMotion(): boolean {
  return (
    typeof window !== "undefined" &&
    typeof window.matchMedia === "function" &&
    window.matchMedia("(prefers-reduced-motion: reduce)").matches
  );
}

export function AuroraOrb({
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

    const colors = [
      resolveToken("--primary"),
      resolveToken("--accent-glow"),
      resolveToken("--accent-glow-end"),
    ];
    const state: AuroraState = { time: 12, intensity: 0 };

    if (reducedMotion) {
      state.intensity = phaseRef.current === "listening" ? 0.4 : 0;
      drawFrame(ctx, size, phaseRef.current, state, colors);
      return;
    }

    let raf = 0;
    const tick = () => {
      stepState(state, phaseRef.current, amplitudeRef.current);
      drawFrame(ctx, size, phaseRef.current, state, colors);
      raf = requestAnimationFrame(tick);
    };
    raf = requestAnimationFrame(tick);

    return () => {
      cancelAnimationFrame(raf);
    };
  }, [size, reducedMotion, staticPhase]);

  return (
    <span
      data-testid="dictation-aurora-orb"
      data-aurora-phase={phase}
      className="relative inline-flex shrink-0"
      style={{ width: size, height: size }}
    >
      <canvas ref={canvasRef} aria-hidden style={{ width: size, height: size }} />
      {phase === "error" && (
        <span
          data-testid="dictation-aurora-error-badge"
          className="bg-destructive absolute -right-0.5 -bottom-0.5 size-2.5 rounded-full border border-black/30"
        />
      )}
    </span>
  );
}

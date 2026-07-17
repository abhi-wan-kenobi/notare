import type { DictationPhase } from "@hypr/plugin-dictation";
import { cn } from "@hypr/utils";

/**
 * "Ring" orb variant: a thin cobalt ring on nothing - the most minimal look
 * that still carries every state. Glow stays a state channel
 * (docs/DESIGN-DIRECTION.md): the ring only lights up while listening.
 *
 * Phase mapping:
 * - idle: faint hairline ring with a slowly orbiting short highlight arc.
 * - listening: the ring thickens and glows with the live mic amplitude and
 *   the highlight arc orbits faster.
 * - processing: a dashed arc spins (spinner-like) while segments flush.
 * - error: desaturated destructive ring + the shared badge dot.
 *
 * Pure SVG + CSS; `prefers-reduced-motion` freezes the arc rotation while
 * amplitude still maps to stroke width/glow so state stays readable.
 */

/** Keeps "listening" visibly alive during speech pauses. */
const LISTENING_FLOOR = 0.08;

const VIEWBOX = 40;
const CENTER = VIEWBOX / 2;
const RADIUS = 14;
const CIRCUMFERENCE = 2 * Math.PI * RADIUS;

export function RingOrb({
  phase,
  amplitude,
  size,
}: {
  phase: DictationPhase;
  amplitude: number;
  size: number;
}) {
  const isError = phase === "error";
  const level =
    phase === "listening" ? Math.max(LISTENING_FLOOR, clamp01(amplitude)) : 0;

  const ringColor = isError ? "hsl(var(--destructive))" : "hsl(var(--primary))";
  const ringWidth = 1.5 + level * 1.8;
  const ringOpacity = isError ? 0.8 : 0.55 + level * 0.45;

  // The orbiting highlight: a short bright arc for idle/listening, a longer
  // dashed arc while processing.
  const arcLength =
    phase === "processing" ? CIRCUMFERENCE * 0.28 : CIRCUMFERENCE * 0.16;

  return (
    <span
      data-testid="dictation-ring-orb"
      data-ring-phase={phase}
      className="relative inline-flex shrink-0 items-center justify-center"
      style={{ width: size, height: size }}
    >
      <svg
        aria-hidden
        viewBox={`0 0 ${VIEWBOX} ${VIEWBOX}`}
        className="absolute inset-0 size-full"
        style={{
          // Glow tracks the voice level only (state channel, not decoration).
          filter:
            level > 0
              ? `drop-shadow(0 0 ${(2 + level * 6).toFixed(1)}px hsl(var(--accent-glow) / ${(0.35 + level * 0.45).toFixed(3)}))`
              : "none",
          transition: "filter 90ms ease-out",
        }}
      >
        {/* Base ring. */}
        <circle
          cx={CENTER}
          cy={CENTER}
          r={RADIUS}
          fill="none"
          stroke={ringColor}
          strokeOpacity={ringOpacity}
          strokeWidth={ringWidth}
          style={{
            transition:
              "stroke-width 90ms ease-out, stroke-opacity 90ms ease-out",
          }}
        />
        {/* Orbiting highlight arc. */}
        {!isError ? (
          <g
            className={cn([
              "origin-center animate-spin motion-reduce:animate-none",
            ])}
            style={{
              animationDuration:
                phase === "processing"
                  ? "1.1s"
                  : phase === "listening"
                    ? "2.4s"
                    : "7s",
            }}
          >
            <circle
              cx={CENTER}
              cy={CENTER}
              r={RADIUS}
              fill="none"
              stroke="hsl(var(--accent-glow))"
              strokeOpacity={phase === "idle" ? 0.5 : 0.9}
              strokeWidth={ringWidth}
              strokeLinecap="round"
              strokeDasharray={`${arcLength} ${CIRCUMFERENCE - arcLength}`}
            />
          </g>
        ) : null}
        {/* Center dot anchors the ring at small sizes. */}
        <circle
          cx={CENTER}
          cy={CENTER}
          r={1.4 + level * 1.2}
          fill={ringColor}
          fillOpacity={isError ? 0.8 : 0.4 + level * 0.6}
          style={{ transition: "r 90ms ease-out, fill-opacity 90ms ease-out" }}
        />
      </svg>
      {isError && (
        <span
          data-testid="dictation-ring-error-badge"
          className="bg-destructive absolute -right-0.5 -bottom-0.5 size-2.5 rounded-full border border-black/30"
        />
      )}
    </span>
  );
}

function clamp01(value: number) {
  if (!Number.isFinite(value)) {
    return 0;
  }

  return Math.min(Math.max(value, 0), 1);
}

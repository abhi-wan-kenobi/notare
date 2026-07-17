import type { DictationPhase } from "@hypr/plugin-dictation";
import { cn } from "@hypr/utils";

/**
 * "Mono" orb variant: a near-static graphite disc with a single tiny state
 * dot - the professional-minimal option for people who want the orb to
 * whisper, not dance.
 *
 * Phase mapping (all on the dot; the disc never moves):
 * - idle: muted dot.
 * - listening: cobalt dot; its scale and glow track the live amplitude -
 *   glow stays a state channel per docs/DESIGN-DIRECTION.md.
 * - processing: cobalt dot with the shared processing pulse.
 * - error: destructive dot (doubles as the error badge at this size).
 *
 * Pure CSS; `prefers-reduced-motion` drops the processing pulse (the dot
 * color alone still tells the state).
 */

/** Keeps "listening" distinguishable from idle during speech pauses. */
const LISTENING_FLOOR = 0.12;

export function MonoOrb({
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

  const dotSize = Math.max(4, Math.round(size * 0.14));
  const dotColor = isError
    ? "hsl(var(--destructive))"
    : phase === "idle"
      ? "hsl(var(--muted-foreground) / 0.7)"
      : "hsl(var(--primary))";

  return (
    <span
      data-testid="dictation-mono-orb"
      data-mono-phase={phase}
      className="relative inline-flex shrink-0 items-center justify-center rounded-full"
      style={{
        width: size,
        height: size,
        // Graphite disc: bg -> surface tokens with a hairline inset border.
        background:
          "radial-gradient(circle at 32% 26%, hsl(var(--card)), hsl(var(--background)) 100%)",
        boxShadow: "inset 0 0 0 1px hsl(var(--border))",
      }}
    >
      <span
        aria-hidden
        data-testid="dictation-mono-dot"
        className={cn([
          "rounded-full",
          phase === "processing" &&
            "animate-orb-pulse motion-reduce:animate-none",
        ])}
        style={{
          width: dotSize,
          height: dotSize,
          background: dotColor,
          transform: `scale(${(1 + level * 0.6).toFixed(3)})`,
          boxShadow:
            level > 0
              ? `0 0 ${Math.round(4 + level * 10)}px hsl(var(--accent-glow) / ${(
                  0.25 +
                  level * 0.45
                ).toFixed(3)})`
              : "none",
          transition:
            "transform 90ms ease-out, box-shadow 90ms ease-out, background 140ms ease",
        }}
      />
    </span>
  );
}

function clamp01(value: number) {
  if (!Number.isFinite(value)) {
    return 0;
  }

  return Math.min(Math.max(value, 0), 1);
}

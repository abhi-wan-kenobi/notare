import type { DictationPhase } from "@hypr/plugin-dictation";
import { DancingSticks } from "@hypr/ui/components/ui/dancing-sticks";
import { cn } from "@hypr/utils";

/**
 * "Pulse" orb variant: the amplitude "dancing sticks" waveform (the same
 * `DancingSticks` animation the meeting header and timeline use) set into a
 * round graphite chassis so it sits alongside the other orb variants.
 *
 * Phase mapping:
 * - idle: flat quiet line (`DancingSticks` at amplitude 0), soft chassis.
 * - listening: sticks dance with the live mic amplitude; the outer glow
 *   tracks the same level (a small floor keeps the sticks visible during
 *   speech pauses so "listening" never reads as "idle").
 * - processing: sticks keep a constant low dance while the whole orb pulses
 *   (same `animate-orb-pulse` treatment as the cobalt variant).
 * - error: flat destructive-tinted line, desaturated chassis, badge dot.
 *
 * `prefers-reduced-motion` freezes every animation (the stick dance and the
 * processing pulse) into a static frame; amplitude still maps to the static
 * stick scale so state stays readable.
 */

/** Keeps the sticks visibly alive during pauses while listening. */
const LISTENING_FLOOR = 0.08;
/** Constant dance level while the final segments flush. */
const PROCESSING_LEVEL = 0.3;

export function WaveformOrb({
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
    phase === "listening"
      ? Math.max(LISTENING_FLOOR, clamp01(amplitude))
      : phase === "processing"
        ? PROCESSING_LEVEL
        : 0;

  return (
    <span
      data-testid="dictation-waveform-orb"
      data-waveform-phase={phase}
      className={cn([
        "relative inline-flex shrink-0 items-center justify-center rounded-full",
        // Reduced motion: freeze the stick dance (and any other descendant
        // animation) into a static frame.
        "motion-reduce:[&_*]:animate-none",
        phase === "processing" &&
          "animate-orb-pulse motion-reduce:animate-none",
      ])}
      style={{ width: size, height: size }}
    >
      {/* Graphite chassis disc (cobalt-core recipe, dimmed for contrast). */}
      <span
        aria-hidden
        className={cn([
          "absolute inset-0 rounded-full",
          isError && "saturate-[0.25]",
        ])}
        style={{
          background:
            "radial-gradient(circle at 32% 26%, color-mix(in oklab, hsl(var(--primary)), black 35%), color-mix(in oklab, hsl(var(--primary)), black 74%) 100%)",
          boxShadow:
            "inset 0 0 0 1px hsl(var(--accent-glow) / 0.3), inset 0 -4px 10px color-mix(in oklab, hsl(var(--primary)), black 55%)",
        }}
      />
      {/* Outer glow - tracks the voice level, off in the error state. */}
      <span
        aria-hidden
        className="absolute inset-0 rounded-full"
        style={{
          boxShadow: isError
            ? "none"
            : `0 0 ${Math.round(8 + level * 22)}px hsl(var(--accent-glow) / ${(
                0.1 +
                level * 0.35
              ).toFixed(3)})`,
          transition: "box-shadow 90ms ease-out",
        }}
      />
      {/* The waveform itself. */}
      <span aria-hidden className="relative inline-flex">
        <DancingSticks
          amplitude={level}
          color={
            isError
              ? "hsl(var(--destructive))"
              : "hsl(var(--accent-glow) / 0.95)"
          }
          width={Math.round(size * 0.55)}
          height={Math.round(size * 0.4)}
          stickWidth={size >= 48 ? 3 : 2}
          gap={size >= 48 ? 2 : 1}
        />
      </span>
      {isError && (
        <span
          data-testid="dictation-waveform-error-badge"
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

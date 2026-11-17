import type { DictationPhase } from "@hypr/plugin-dictation";
import { DancingSticks } from "@hypr/ui/components/ui/dancing-sticks";
import { cn } from "@hypr/utils";

/**
 * Original compact capsule: neutral shell with a voice-reactive waveform.
 * Colour stays inside the waveform, never in the surrounding chrome.
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
  const stickWidth = Math.max(1, Math.round(size / 10));
  const gap = Math.max(1, Math.round(size / 15));

  return (
    <span
      data-testid="dictation-waveform-orb"
      data-waveform-phase={phase}
      className={cn([
        "bg-foreground/8 border-foreground/15 relative inline-flex shrink-0 items-center justify-center overflow-hidden rounded-full border",
        // Reduced motion: freeze the stick dance (and any other descendant
        // animation) into a static frame.
        "motion-reduce:[&_*]:animate-none",
        phase === "processing" &&
          "animate-orb-pulse motion-reduce:animate-none",
      ])}
      style={{ width: Math.round(size * (62 / 30)), height: size }}
    >
      {/* The waveform itself. */}
      <span aria-hidden className="relative inline-flex">
        <DancingSticks
          amplitude={level}
          color={
            isError
              ? "hsl(var(--destructive))"
              : "hsl(var(--accent-glow) / 0.95)"
          }
          width={5 * stickWidth + 4 * gap}
          height={Math.round(size * (20 / 30))}
          stickWidth={stickWidth}
          gap={gap}
        />
      </span>
      {isError && (
        <span
          data-testid="dictation-waveform-error-badge"
          className="bg-foreground absolute right-1.5 bottom-1.5 size-1 rounded-full"
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

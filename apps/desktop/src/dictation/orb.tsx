import type { DictationPhase } from "@hypr/plugin-dictation";
import { cn } from "@hypr/utils";

import { WaveformOrb } from "./waveform-orb";

/**
 * Shared compact waveform for meeting recording and live dictation.
 * The shell and status overlays stay monochrome; only the waveform is tinted.
 */
export function DictationOrb({
  phase,
  amplitude = 0,
  size = 30,
  className,
}: {
  phase: DictationPhase;
  amplitude?: number;
  size?: number;
  className?: string;
}) {
  return (
    <span
      data-testid="dictation-orb"
      data-dictation-phase={phase}
      className={cn(["relative inline-flex", className])}
    >
      <WaveformOrb phase={phase} amplitude={amplitude} size={size} />
      <PhaseOverlay phase={phase} size={size} />
    </span>
  );
}

/** Shared processing and completion affordances. */
function PhaseOverlay({
  phase,
  size,
}: {
  phase: DictationPhase;
  size: number;
}) {
  switch (phase) {
    case "processing":
      return <ProcessingRing size={size} />;
    case "success":
      return <SuccessCheck size={size} />;
    case "idle":
    case "listening":
    case "error":
      return null;
    default:
      return assertNever(phase);
  }
}

/**
 * Compile-time exhaustiveness guard: reachable only if a `DictationPhase`
 * variant is left unhandled above (then `phase` is not `never` and this errors
 * at the call site). Returns `null` so that if the bindings ever drift ahead of
 * the UI at runtime, an unknown phase renders nothing rather than throwing.
 */
function assertNever(_phase: never): null {
  return null;
}

/** Processing remains visible while the final segments flush. */
function ProcessingRing({ size }: { size: number }) {
  const ring = Math.round(size * 1.18);
  return (
    <span
      aria-hidden
      data-testid="dictation-orb-processing"
      className="text-foreground/75 pointer-events-none absolute inset-0 flex items-center justify-center"
    >
      <svg
        width={ring}
        height={ring}
        viewBox="0 0 100 100"
        className="animate-spin motion-reduce:animate-none"
        style={{ animationDuration: "1.1s" }}
      >
        <circle
          cx="50"
          cy="50"
          r="45"
          fill="none"
          stroke="currentColor"
          strokeWidth="5"
          strokeLinecap="round"
          strokeDasharray="64 240"
        />
      </svg>
    </span>
  );
}

/** Completion confirmation; honors reduced motion. */
function SuccessCheck({ size }: { size: number }) {
  const ring = Math.round(size * 1.18);
  return (
    <span
      aria-hidden
      data-testid="dictation-orb-success"
      className="animate-orb-success text-foreground pointer-events-none absolute inset-0 flex items-center justify-center motion-reduce:animate-none"
    >
      <svg width={ring} height={ring} viewBox="0 0 100 100">
        <circle
          cx="50"
          cy="50"
          r="45"
          fill="none"
          stroke="currentColor"
          strokeWidth="5"
          strokeOpacity="0.9"
        />
        <path
          d="M30 52 L45 66 L71 36"
          fill="none"
          stroke="currentColor"
          strokeWidth="7"
          strokeLinecap="round"
          strokeLinejoin="round"
        />
      </svg>
    </span>
  );
}

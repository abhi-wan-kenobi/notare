import type { ComponentType } from "react";

import type { DictationPhase } from "@hypr/plugin-dictation";
import { cn } from "@hypr/utils";

import { RecordingOrb } from "~/meeting-float/orb";

/**
 * Available orb looks. One style ships today; the component is structured so
 * a future style picker only has to add entries to `ORB_VARIANTS`.
 */
export type DictationOrbVariant = "cobalt";

export const DEFAULT_ORB_VARIANT: DictationOrbVariant = "cobalt";

export interface DictationOrbVariantProps {
  phase: DictationPhase;
  amplitude: number;
  size: number;
}

/**
 * The mini dictation orb: a small persistent variant of the meeting orb
 * (docs/DESIGN-DIRECTION.md §3b, Cobalt-on-graphite). Phase mapping:
 *
 * - idle: matte core, slow rim drift (orb visible, not dictating).
 * - listening: rim glow + liquid level track the mic amplitude.
 * - processing: idle core with a pulse while the final segments flush.
 * - error: desaturated core with the destructive badge dot.
 */
export function DictationOrb({
  phase,
  amplitude = 0,
  size = 40,
  variant = DEFAULT_ORB_VARIANT,
  className,
}: {
  phase: DictationPhase;
  amplitude?: number;
  size?: number;
  variant?: DictationOrbVariant;
  className?: string;
}) {
  const Variant = ORB_VARIANTS[variant] ?? CobaltOrb;

  return (
    <span
      data-testid="dictation-orb"
      data-dictation-phase={phase}
      data-dictation-variant={variant}
      className={cn(["inline-flex", className])}
    >
      <Variant phase={phase} amplitude={amplitude} size={size} />
    </span>
  );
}

const ORB_VARIANTS: Record<
  DictationOrbVariant,
  ComponentType<DictationOrbVariantProps>
> = {
  cobalt: CobaltOrb,
};

function CobaltOrb({ phase, amplitude, size }: DictationOrbVariantProps) {
  const orbState =
    phase === "error" ? "error" : phase === "listening" ? "listening" : "idle";

  return (
    <span
      className={cn([
        "inline-flex",
        phase === "processing" &&
          "animate-orb-pulse motion-reduce:animate-none",
      ])}
    >
      <RecordingOrb state={orbState} amplitude={amplitude} size={size} />
    </span>
  );
}

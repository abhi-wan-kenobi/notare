import type { ComponentType } from "react";

import type { DictationPhase } from "@hypr/plugin-dictation";
import { cn } from "@hypr/utils";

import { RecordingOrb } from "~/meeting-float/orb";

import { ParticleOrb } from "./particle-orb";
import { WaveformOrb } from "./waveform-orb";

/**
 * Available orb looks, selected by the `dictation_orb_variant` setting:
 * - "cobalt": the mini meeting orb (Cobalt-on-graphite);
 * - "particles": the voice-reactive particle sphere (`particle-orb.tsx`);
 * - "waveform": "Pulse" - the dancing-sticks amplitude waveform in a round
 *   chassis (`waveform-orb.tsx`).
 */
export type DictationOrbVariant = "cobalt" | "particles" | "waveform";

export const DEFAULT_ORB_VARIANT: DictationOrbVariant = "cobalt";

/** Map whatever the settings store holds onto a known variant. */
export function normalizeOrbVariant(
  raw: string | undefined,
): DictationOrbVariant {
  return raw === "particles" || raw === "waveform"
    ? raw
    : DEFAULT_ORB_VARIANT;
}

/**
 * Per-variant render scale over the caller's base size. The particle sphere
 * reads too small at the cobalt size (most of its extent is a faint halo),
 * so it renders 1.5x larger everywhere - orb window and settings previews
 * alike.
 */
const ORB_VARIANT_SCALE: Record<DictationOrbVariant, number> = {
  cobalt: 1,
  particles: 1.5,
  waveform: 1,
};

/** Orb pixel size for `variant`, scaled from the caller's base size. */
export function orbSizeForVariant(
  variant: DictationOrbVariant,
  baseSize: number,
): number {
  return Math.round(baseSize * (ORB_VARIANT_SCALE[variant] ?? 1));
}

/**
 * Mirror of `ORB_SIZE` in `plugins/dictation/src/orb.rs`: the Rust side
 * always creates the orb window at the cobalt chassis size and the orb
 * webview resizes itself per variant (it is the one that knows the setting).
 */
const ORB_WINDOW_BASE_SIZE = 56;

/** Logical size of the dictation orb window for `variant`. */
export function orbWindowSizeForVariant(variant: DictationOrbVariant): number {
  return Math.round(ORB_WINDOW_BASE_SIZE * (ORB_VARIANT_SCALE[variant] ?? 1));
}

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
      <Variant
        phase={phase}
        amplitude={amplitude}
        size={orbSizeForVariant(variant, size)}
      />
    </span>
  );
}

const ORB_VARIANTS: Record<
  DictationOrbVariant,
  ComponentType<DictationOrbVariantProps>
> = {
  cobalt: CobaltOrb,
  particles: ParticleOrb,
  waveform: WaveformOrb,
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

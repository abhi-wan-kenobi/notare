import { Trans } from "@lingui/react/macro";
import type { ComponentType, ReactNode } from "react";

import type { DictationPhase } from "@hypr/plugin-dictation";
import { cn } from "@hypr/utils";

import { AuroraOrb } from "./aurora-orb";
import { BloomOrb } from "./bloom-orb";
import { EmberOrb } from "./ember-orb";
import { HaloOrb } from "./halo-orb";
import { MonoOrb } from "./mono-orb";
import { ParticleOrb } from "./particle-orb";
import { PipOrb } from "./pip-orb";
import { RingOrb } from "./ring-orb";
import { SilkOrb } from "./silk-orb";
import { WaveformOrb } from "./waveform-orb";

import { RecordingOrb } from "~/meeting-float/orb";

/**
 * Available orb looks, selected by the `dictation_orb_variant` setting:
 * - "cobalt": the mini meeting orb (Cobalt-on-graphite);
 * - "particles": the voice-reactive particle sphere (`particle-orb.tsx`);
 * - "waveform": "Pulse" - the dancing-sticks amplitude waveform in a round
 *   chassis (`waveform-orb.tsx`);
 * - "ring": a thin cobalt ring, amplitude = stroke pulse (`ring-orb.tsx`);
 * - "aurora": soft drifting gradient blobs (`aurora-orb.tsx`);
 * - "mono": a near-static graphite disc + state dot (`mono-orb.tsx`);
 * - "bloom": warm amber petals that swell with your voice (`bloom-orb.tsx`);
 * - "halo": a neon cyan-magenta rim ringed with voice-reactive beads
 *   (`halo-orb.tsx`);
 * - "ember": a dark glass sphere with a hot magenta caustic band
 *   (`ember-orb.tsx`);
 * - "silk": a soft lavender ball with turning combed striations
 *   (`silk-orb.tsx`);
 * - "pip": a friendly squishy blob that reacts through expression
 *   (`pip-orb.tsx`).
 *
 * Adding a variant here (union + `ORB_VARIANT_REGISTRY` entry) is all it
 * takes: the settings picker (`OrbVariantGroup`) renders from the registry.
 */
export type DictationOrbVariant =
  | "cobalt"
  | "particles"
  | "waveform"
  | "ring"
  | "aurora"
  | "mono"
  | "bloom"
  | "halo"
  | "ember"
  | "silk"
  | "pip";

export const DEFAULT_ORB_VARIANT: DictationOrbVariant = "cobalt";

/** Map whatever the settings store holds onto a known variant. */
export function normalizeOrbVariant(
  raw: string | undefined,
): DictationOrbVariant {
  return raw != null && raw in ORB_VARIANT_REGISTRY
    ? (raw as DictationOrbVariant)
    : DEFAULT_ORB_VARIANT;
}

/**
 * Per-variant render scale over the caller's base size. The particle sphere
 * reads too small at the cobalt size (most of its extent is a faint halo),
 * so it renders 1.5x larger everywhere - orb window and settings previews
 * alike.
 */
export const ORB_VARIANT_SCALE: Record<DictationOrbVariant, number> = {
  cobalt: 1,
  particles: 1.5,
  waveform: 1,
  ring: 1,
  aurora: 1,
  mono: 1,
  bloom: 1,
  halo: 1,
  ember: 1,
  silk: 1,
  pip: 1,
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

export interface OrbVariantInfo {
  component: ComponentType<DictationOrbVariantProps>;
  /** Display name in the settings picker. */
  title: ReactNode;
  /** One-line descriptor in the settings picker. */
  description: ReactNode;
}

/**
 * The single source of truth for orb variants: rendering (`DictationOrb`)
 * and the settings picker both read from here, so a new entry shows up in
 * the picker automatically.
 */
export const ORB_VARIANT_REGISTRY: Record<DictationOrbVariant, OrbVariantInfo> =
  {
    cobalt: {
      component: CobaltOrb,
      title: <Trans>Cobalt</Trans>,
      description: <Trans>The minimal glowing orb.</Trans>,
    },
    particles: {
      component: ParticleOrb,
      title: <Trans>Particles</Trans>,
      description: <Trans>A voice-reactive particle sphere.</Trans>,
    },
    waveform: {
      component: WaveformOrb,
      title: <Trans>Pulse</Trans>,
      description: <Trans>A waveform of bars that dance as you speak.</Trans>,
    },
    ring: {
      component: RingOrb,
      title: <Trans>Ring</Trans>,
      description: <Trans>A thin cobalt ring that pulses as you speak.</Trans>,
    },
    aurora: {
      component: AuroraOrb,
      title: <Trans>Aurora</Trans>,
      description: (
        <Trans>Soft drifting color that brightens with your voice.</Trans>
      ),
    },
    mono: {
      component: MonoOrb,
      title: <Trans>Mono</Trans>,
      description: (
        <Trans>A quiet graphite disc with a single state dot.</Trans>
      ),
    },
    bloom: {
      component: BloomOrb,
      title: <Trans>Bloom</Trans>,
      description: (
        <Trans>Warm petals that bloom and breathe with your voice.</Trans>
      ),
    },
    halo: {
      component: HaloOrb,
      title: <Trans>Halo</Trans>,
      description: <Trans>A neon rim ringed with voice-reactive beads.</Trans>,
    },
    ember: {
      component: EmberOrb,
      title: <Trans>Ember</Trans>,
      description: (
        <Trans>A dark glass sphere lit by a hot caustic band.</Trans>
      ),
    },
    silk: {
      component: SilkOrb,
      title: <Trans>Silk</Trans>,
      description: (
        <Trans>Soft lavender striations turning over a sphere.</Trans>
      ),
    },
    pip: {
      component: PipOrb,
      title: <Trans>Pip</Trans>,
      description: (
        <Trans>A friendly blob that listens through its face.</Trans>
      ),
    },
  };

/** Picker order (matches the registry's declaration order). */
export const ORB_VARIANT_ORDER = Object.keys(
  ORB_VARIANT_REGISTRY,
) as DictationOrbVariant[];

const ORB_VARIANTS: Record<
  DictationOrbVariant,
  ComponentType<DictationOrbVariantProps>
> = Object.fromEntries(
  Object.entries(ORB_VARIANT_REGISTRY).map(([variant, info]) => [
    variant,
    info.component,
  ]),
) as Record<DictationOrbVariant, ComponentType<DictationOrbVariantProps>>;

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

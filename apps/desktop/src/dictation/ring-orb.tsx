import { useEffect, useRef } from "react";

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
 * When the orb window hands us its ~30 Hz `amplitudeRef`, the ring follows the
 * mic smoothly via a requestAnimationFrame envelope follower (fast attack 0.15
 * / slow decay 0.04, mirroring `particle-orb.tsx`) that writes stroke width /
 * opacity / glow / arc rotation straight to the SVG nodes - no React state, no
 * re-render per frame. Otherwise (reduced motion, or no ref - settings preview,
 * the meeting-float orb) the pure-SVG render below maps the 10 Hz `amplitude`
 * prop directly, as before. `prefers-reduced-motion` always takes the static
 * path; there the arc rotation freezes while amplitude still maps to stroke
 * width/glow so state stays readable.
 */

/** Keeps "listening" visibly alive during speech pauses. */
const LISTENING_FLOOR = 0.08;

const VIEWBOX = 40;
const CENTER = VIEWBOX / 2;
const RADIUS = 14;
const CIRCUMFERENCE = 2 * Math.PI * RADIUS;

// Envelope: fast attack, slow decay - same constants as particle-orb's
// `stepState`, so the ring swells and fades with the same feel as the sphere.
const ATTACK = 0.15;
const DECAY = 0.04;
/** Below this the envelope is considered settled -> the rAF loop pauses. */
const SETTLED_EPSILON = 0.001;

// Orbiting highlight arc rotation (radians/frame at ~60fps), tuned to the
// static CSS durations so the animated path matches the fallback look. Only
// the listening arc is rAF-driven (it speeds up with voice); idle/processing
// keep their CSS `animate-spin` so the loop can stop while idle.
const ARC_SPEED_LISTENING = (2 * Math.PI) / (2.4 * 60);

export function RingOrb({
  phase,
  amplitude,
  size,
  amplitudeRef,
}: {
  phase: DictationPhase;
  amplitude: number;
  size: number;
  amplitudeRef?: { current: number };
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

  const reducedMotion = prefersReducedMotion();
  // Imperative 30 Hz path only when the window feeds us a ref AND motion is
  // allowed. Otherwise the pure-SVG render below is the whole story.
  const animate = !reducedMotion && amplitudeRef != null;

  const svgRef = useRef<SVGSVGElement | null>(null);
  const ringRef = useRef<SVGCircleElement | null>(null);
  const dotRef = useRef<SVGCircleElement | null>(null);
  const arcGroupRef = useRef<SVGGElement | null>(null);
  const arcCircleRef = useRef<SVGCircleElement | null>(null);

  const phaseRef = useRef(phase);
  phaseRef.current = phase;
  // Mirror the (stable) ref prop into a ref so the rAF closure never captures
  // a React prop as a dependency - same idiom as particle-orb's amplitudeRef.
  const ampSourceRef = useRef(amplitudeRef);
  ampSourceRef.current = amplitudeRef;

  const intensityRef = useRef(0);
  const arcAngleRef = useRef(0);
  const rafRef = useRef(0);
  const tickRef = useRef<() => void>(() => {});

  useEffect(() => {
    if (!animate) {
      return;
    }
    const svg = svgRef.current;
    const ring = ringRef.current;
    if (!svg || !ring) {
      return;
    }
    const dot = dotRef.current;
    const arcGroup = arcGroupRef.current;
    const arcCircle = arcCircleRef.current;

    // Write every level-encoded attribute straight to the DOM - no setState,
    // no re-render. Mirrors the static render's formulas with the envelope
    // `intensity` in place of the raw `level`.
    const apply = (intensity: number) => {
      const p = phaseRef.current;
      const listening = p === "listening";
      const err = p === "error";
      // The listening floor is a display floor only: the envelope itself can
      // decay to zero during a pause while the ring stays visibly alive.
      const displayLevel = listening
        ? Math.max(LISTENING_FLOOR, intensity)
        : intensity;
      const width = 1.5 + displayLevel * 1.8;
      const opacity = err ? 0.8 : 0.55 + displayLevel * 0.45;

      ring.setAttribute("stroke-width", width.toFixed(3));
      ring.setAttribute("stroke-opacity", opacity.toFixed(3));
      svg.style.filter =
        displayLevel > 0
          ? `drop-shadow(0 0 ${(2 + displayLevel * 6).toFixed(1)}px hsl(var(--accent-glow) / ${(0.35 + displayLevel * 0.45).toFixed(3)}))`
          : "none";

      if (dot) {
        dot.setAttribute("r", (1.4 + displayLevel * 1.2).toFixed(3));
        dot.setAttribute(
          "fill-opacity",
          (err ? 0.8 : 0.4 + displayLevel * 0.6).toFixed(3),
        );
      }
      if (arcCircle) {
        arcCircle.setAttribute("stroke-width", width.toFixed(3));
      }
    };

    const tick = () => {
      const p = phaseRef.current;
      const listening = p === "listening";
      const target = listening
        ? clamp01(ampSourceRef.current?.current ?? 0)
        : 0;

      const prev = intensityRef.current;
      const next = prev + (target - prev) * (target > prev ? ATTACK : DECAY);
      intensityRef.current = next;
      apply(next);

      if (listening && arcGroup) {
        // Faster orbit with voice, like the particle sphere's `rotY`.
        arcAngleRef.current += ARC_SPEED_LISTENING * (1 + next * 1.5);
        arcGroup.style.transform = `rotate(${arcAngleRef.current}rad)`;
      } else if (arcGroup) {
        // Non-listening: CSS `animate-spin` owns the arc. Release the inline
        // transform so it isn't left overriding the CSS animation.
        arcGroup.style.transform = "";
      }

      // No reactive work outside listening once the envelope has settled to
      // its zero target - stop scheduling. This is what keeps rAF at zero
      // while idle/processing/error (the arc, if any, is CSS-driven then).
      if (!listening && next < SETTLED_EPSILON) {
        rafRef.current = 0;
        return;
      }
      rafRef.current = requestAnimationFrame(tick);
    };
    tickRef.current = tick;

    // Seed the imperative attributes so the first painted frame isn't at the
    // browser defaults (stroke-width 1); the loop then takes over at 30 Hz.
    apply(intensityRef.current);
    if (!rafRef.current) {
      rafRef.current = requestAnimationFrame(tick);
    }

    return () => {
      if (rafRef.current) {
        cancelAnimationFrame(rafRef.current);
      }
      rafRef.current = 0;
    };
  }, [animate, size]);

  // The loop pauses itself once idle. A phase change into an animated state
  // (listening) is the signal to wake it - React re-renders on phase changes
  // (the 10 Hz state cadence), so this effect re-schedules one frame iff none
  // is already pending.
  useEffect(() => {
    if (!animate || rafRef.current) {
      return;
    }
    rafRef.current = requestAnimationFrame(tickRef.current);
  }, [phase, animate]);

  // When the imperative path owns the level-encoded attributes, React must not
  // also write them from the 10 Hz `amplitude` prop (that would re-introduce
  // the 10 Hz jitter the envelope is here to smooth). So in `animate` mode we
  // omit those attributes/style from JSX and let the refs drive them; the
  // static fallback renders them exactly as before.
  return (
    <span
      data-testid="dictation-ring-orb"
      data-ring-phase={phase}
      className="relative inline-flex shrink-0 items-center justify-center"
      style={{ width: size, height: size }}
    >
      <svg
        ref={animate ? svgRef : undefined}
        aria-hidden
        viewBox={`0 0 ${VIEWBOX} ${VIEWBOX}`}
        className="absolute inset-0 size-full"
        style={
          animate
            ? undefined
            : {
                // Glow tracks the voice level only (state channel, not decoration).
                filter:
                  level > 0
                    ? `drop-shadow(0 0 ${(2 + level * 6).toFixed(1)}px hsl(var(--accent-glow) / ${(0.35 + level * 0.45).toFixed(3)}))`
                    : "none",
                transition: "filter 90ms ease-out",
              }
        }
      >
        {/* Base ring. */}
        <circle
          ref={animate ? ringRef : undefined}
          cx={CENTER}
          cy={CENTER}
          r={RADIUS}
          fill="none"
          stroke={ringColor}
          strokeOpacity={animate ? undefined : ringOpacity}
          strokeWidth={animate ? undefined : ringWidth}
          style={
            animate
              ? undefined
              : {
                  transition:
                    "stroke-width 90ms ease-out, stroke-opacity 90ms ease-out",
                }
          }
        />
        {/* Orbiting highlight arc. */}
        {!isError ? (
          <g
            ref={animate ? arcGroupRef : undefined}
            className={cn([
              "origin-center",
              // CSS spin owns the arc except when the rAF loop is driving it
              // (listening with a 30 Hz ref); `motion-reduce:animate-none`
              // freezes it for reduced motion in the static fallback.
              !(animate && phase === "listening") &&
                "animate-spin motion-reduce:animate-none",
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
              ref={animate ? arcCircleRef : undefined}
              cx={CENTER}
              cy={CENTER}
              r={RADIUS}
              fill="none"
              stroke="hsl(var(--accent-glow))"
              strokeOpacity={phase === "idle" ? 0.5 : 0.9}
              strokeWidth={animate ? undefined : ringWidth}
              strokeLinecap="round"
              strokeDasharray={`${arcLength} ${CIRCUMFERENCE - arcLength}`}
            />
          </g>
        ) : null}
        {/* Center dot anchors the ring at small sizes. */}
        <circle
          ref={animate ? dotRef : undefined}
          cx={CENTER}
          cy={CENTER}
          r={animate ? undefined : 1.4 + level * 1.2}
          fill={ringColor}
          fillOpacity={animate ? undefined : isError ? 0.8 : 0.4 + level * 0.6}
          style={
            animate
              ? undefined
              : { transition: "r 90ms ease-out, fill-opacity 90ms ease-out" }
          }
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

function prefersReducedMotion(): boolean {
  return (
    typeof window !== "undefined" &&
    typeof window.matchMedia === "function" &&
    window.matchMedia("(prefers-reduced-motion: reduce)").matches
  );
}

function clamp01(value: number) {
  if (!Number.isFinite(value)) {
    return 0;
  }

  return Math.min(Math.max(value, 0), 1);
}

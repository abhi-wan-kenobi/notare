import { cn } from "@hypr/utils";

export type RecordingOrbState = "idle" | "listening" | "error";

/**
 * The Notare orb (docs/DESIGN-DIRECTION.md §3b, reference R4): an iridescent
 * glass sphere built from layered CSS gradients — no canvas, no video.
 *
 * - idle: matte cobalt core, slow rim drift.
 * - listening: rim glows and the halo tracks the input amplitude.
 * - error: desaturated core with a destructive badge dot.
 *
 * `prefers-reduced-motion` freezes the rim drift; amplitude still maps to a
 * static glow level so state stays readable.
 */
export function RecordingOrb({
  state,
  amplitude = 0,
  size = 34,
  className,
}: {
  state: RecordingOrbState;
  amplitude?: number;
  size?: number;
  className?: string;
}) {
  const level = state === "listening" ? clamp01(amplitude) : 0;
  const isError = state === "error";

  return (
    <span
      data-testid="recording-orb"
      data-orb-state={state}
      className={cn(["relative inline-flex shrink-0", className])}
      style={{ width: size, height: size }}
    >
      {/* Iridescent rim light: a conic sweep blurred into a halo. */}
      <span
        aria-hidden
        className={cn([
          "animate-orb-spin absolute -inset-0.5 rounded-full blur-[5px] motion-reduce:animate-none",
          "transition-opacity duration-(--motion-duration-panel)",
          isError && "opacity-15",
        ])}
        style={{
          background:
            "conic-gradient(from 40deg, hsl(var(--accent-glow) / 0.9), hsl(var(--primary) / 0.8), hsl(var(--accent-glow-end) / 0.9), hsl(var(--primary) / 0.7), hsl(var(--accent-glow) / 0.9))",
          opacity: isError ? undefined : 0.45 + level * 0.55,
        }}
      />
      {/* Glass shell + cobalt liquid core. */}
      <span
        aria-hidden
        className={cn([
          "absolute inset-0 overflow-hidden rounded-full",
          isError && "saturate-[0.25]",
        ])}
        style={{
          background:
            "radial-gradient(circle at 32% 26%, hsl(var(--accent-glow) / 0.85), hsl(var(--primary)) 42%, color-mix(in oklab, hsl(var(--primary)), black 62%) 100%)",
          boxShadow:
            "inset 0 0 0 1px hsl(var(--accent-glow) / 0.35), inset 0 -4px 10px color-mix(in oklab, hsl(var(--primary)), black 55%)",
          transform: `scale(${1 + level * 0.05})`,
          transition: "transform 80ms ease-out",
        }}
      >
        {/* Liquid level: rises with input amplitude while listening. */}
        <span
          aria-hidden
          className="absolute inset-x-0 bottom-0"
          style={{
            height: `${22 + level * 58}%`,
            background:
              "linear-gradient(to top, hsl(var(--accent-glow) / 0.55), hsl(var(--accent-glow-end) / 0.18) 70%, transparent)",
            transition: "height 90ms ease-out",
          }}
        />
        {/* Specular highlight. */}
        <span
          aria-hidden
          className="absolute rounded-full"
          style={{
            top: "12%",
            left: "18%",
            width: "34%",
            height: "26%",
            background:
              "radial-gradient(circle, rgba(255, 255, 255, 0.85), transparent 70%)",
          }}
        />
      </span>
      {/* Outer glow — live states only (§2 glow recipe). */}
      <span
        aria-hidden
        className="absolute inset-0 rounded-full"
        style={{
          boxShadow: isError
            ? "none"
            : `0 0 ${Math.round(10 + level * 26)}px hsl(var(--accent-glow) / ${(
                0.12 +
                level * 0.38
              ).toFixed(3)})`,
        }}
      />
      {isError && (
        <span
          data-testid="recording-orb-error-badge"
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

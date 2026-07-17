import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";

import { RingOrb } from "./ring-orb";

function baseRing(container: HTMLElement) {
  return container.querySelector("circle");
}

describe("RingOrb", () => {
  afterEach(() => {
    cleanup();
  });

  it("renders at the requested size", () => {
    render(<RingOrb phase="idle" amplitude={0} size={56} />);

    const orb = screen.getByTestId("dictation-ring-orb");
    expect(orb.style.width).toBe("56px");
    expect(orb.style.height).toBe("56px");
    expect(orb.dataset.ringPhase).toBe("idle");
  });

  it("shows a faint hairline ring with no glow while idle", () => {
    const { container } = render(
      <RingOrb phase="idle" amplitude={0} size={56} />,
    );

    const ring = baseRing(container);
    expect(ring?.getAttribute("stroke-width")).toBe("1.5");
    const svg = container.querySelector("svg");
    expect(svg?.style.filter).toBe("none");
  });

  it("thickens and glows with the amplitude while listening", () => {
    const { container } = render(
      <RingOrb phase="listening" amplitude={0.8} size={56} />,
    );

    const ring = baseRing(container);
    expect(Number(ring?.getAttribute("stroke-width"))).toBeGreaterThan(1.5);
    const svg = container.querySelector("svg");
    expect(svg?.style.filter).toContain("drop-shadow");
  });

  it("keeps a visible floor during listening pauses", () => {
    const { container } = render(
      <RingOrb phase="listening" amplitude={0} size={56} />,
    );

    const svg = container.querySelector("svg");
    expect(svg?.style.filter).toContain("drop-shadow");
  });

  it("spins the highlight arc while processing", () => {
    const { container } = render(
      <RingOrb phase="processing" amplitude={0} size={56} />,
    );

    const spinner = container.querySelector("g.animate-spin");
    expect(spinner).not.toBeNull();
    expect((spinner as SVGGElement).className.baseVal).toContain(
      "motion-reduce:animate-none",
    );
    expect((spinner as SVGGElement).style.animationDuration).toBe("1.1s");
  });

  it("drops the arc and shows the badge dot on error", () => {
    const { container } = render(
      <RingOrb phase="error" amplitude={0.8} size={56} />,
    );

    expect(container.querySelector("g.animate-spin")).toBeNull();
    expect(screen.getByTestId("dictation-ring-error-badge")).not.toBeNull();
    expect(baseRing(container)?.getAttribute("stroke")).toBe(
      "hsl(var(--destructive))",
    );
  });
});

import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";

import { MonoOrb } from "./mono-orb";

describe("MonoOrb", () => {
  afterEach(() => {
    cleanup();
  });

  it("renders a static disc at the requested size", () => {
    render(<MonoOrb phase="idle" amplitude={0} size={56} />);

    const orb = screen.getByTestId("dictation-mono-orb");
    expect(orb.style.width).toBe("56px");
    expect(orb.style.height).toBe("56px");
    expect(orb.className).toContain("rounded-full");
    expect(orb.dataset.monoPhase).toBe("idle");
  });

  it("shows a muted dot with no glow while idle", () => {
    render(<MonoOrb phase="idle" amplitude={0} size={56} />);

    const dot = screen.getByTestId("dictation-mono-dot");
    expect(dot.style.background).toContain("muted-foreground");
    expect(dot.style.boxShadow).toBe("none");
  });

  it("turns cobalt, scales and glows with amplitude while listening", () => {
    render(<MonoOrb phase="listening" amplitude={0.8} size={56} />);

    const dot = screen.getByTestId("dictation-mono-dot");
    expect(dot.style.background).toContain("--primary");
    expect(dot.style.transform).not.toBe("scale(1.000)");
    expect(dot.style.boxShadow).toContain("accent-glow");
  });

  it("keeps a visible floor during listening pauses", () => {
    render(<MonoOrb phase="listening" amplitude={0} size={56} />);

    const dot = screen.getByTestId("dictation-mono-dot");
    expect(dot.style.boxShadow).toContain("accent-glow");
  });

  it("pulses the dot while processing", () => {
    render(<MonoOrb phase="processing" amplitude={0} size={56} />);

    const dot = screen.getByTestId("dictation-mono-dot");
    expect(dot.className).toContain("animate-orb-pulse");
    expect(dot.className).toContain("motion-reduce:animate-none");
  });

  it("shows a destructive dot on error", () => {
    render(<MonoOrb phase="error" amplitude={0.5} size={56} />);

    const dot = screen.getByTestId("dictation-mono-dot");
    expect(dot.style.background).toContain("--destructive");
    expect(dot.style.boxShadow).toBe("none");
  });
});

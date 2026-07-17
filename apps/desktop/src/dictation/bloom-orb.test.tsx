import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";

import { BloomOrb } from "./bloom-orb";

/**
 * jsdom has no canvas (test-setup returns a null 2D context), so these tests
 * cover the component contract - sizing, phase attribution, error badge -
 * while the renderer itself stays behind the null-context guard.
 */
describe("BloomOrb", () => {
  afterEach(() => {
    cleanup();
  });

  it("renders at the requested size with a canvas", () => {
    render(<BloomOrb phase="idle" amplitude={0} size={56} />);

    const orb = screen.getByTestId("dictation-bloom-orb");
    expect(orb.style.width).toBe("56px");
    expect(orb.style.height).toBe("56px");
    expect(orb.dataset.bloomPhase).toBe("idle");

    const canvas = orb.querySelector("canvas");
    expect(canvas).not.toBeNull();
    expect(canvas?.style.width).toBe("56px");
  });

  it("tracks the phase attribute for every phase", () => {
    for (const phase of ["idle", "listening", "processing", "error"] as const) {
      cleanup();
      render(<BloomOrb phase={phase} amplitude={0.4} size={56} />);
      expect(screen.getByTestId("dictation-bloom-orb").dataset.bloomPhase).toBe(
        phase,
      );
    }
  });

  it("shows the badge dot on error only", () => {
    render(<BloomOrb phase="error" amplitude={0} size={56} />);
    expect(screen.getByTestId("dictation-bloom-error-badge")).not.toBeNull();

    cleanup();
    render(<BloomOrb phase="listening" amplitude={0.6} size={56} />);
    expect(screen.queryByTestId("dictation-bloom-error-badge")).toBeNull();
  });
});

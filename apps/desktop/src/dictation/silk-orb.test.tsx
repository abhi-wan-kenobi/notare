import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";

import { SilkOrb } from "./silk-orb";

/**
 * jsdom has no canvas (test-setup returns a null 2D context), so these tests
 * cover the component contract - sizing, phase attribution, error badge -
 * while the renderer itself stays behind the null-context guard.
 */
describe("SilkOrb", () => {
  afterEach(() => {
    cleanup();
  });

  it("renders at the requested size with a canvas", () => {
    render(<SilkOrb phase="idle" amplitude={0} size={56} />);

    const orb = screen.getByTestId("dictation-silk-orb");
    expect(orb.style.width).toBe("56px");
    expect(orb.style.height).toBe("56px");
    expect(orb.dataset.silkPhase).toBe("idle");

    const canvas = orb.querySelector("canvas");
    expect(canvas).not.toBeNull();
    expect(canvas?.style.width).toBe("56px");
  });

  it("tracks the phase attribute for every phase", () => {
    for (const phase of ["idle", "listening", "processing", "error"] as const) {
      cleanup();
      render(<SilkOrb phase={phase} amplitude={0.4} size={56} />);
      expect(screen.getByTestId("dictation-silk-orb").dataset.silkPhase).toBe(
        phase,
      );
    }
  });

  it("shows the badge dot on error only", () => {
    render(<SilkOrb phase="error" amplitude={0} size={56} />);
    expect(screen.getByTestId("dictation-silk-error-badge")).not.toBeNull();

    cleanup();
    render(<SilkOrb phase="listening" amplitude={0.6} size={56} />);
    expect(screen.queryByTestId("dictation-silk-error-badge")).toBeNull();
  });
});

import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";

import { EmberOrb } from "./ember-orb";

/**
 * jsdom has no canvas (test-setup returns a null 2D context), so these tests
 * cover the component contract - sizing, phase attribution, error badge -
 * while the renderer itself stays behind the null-context guard.
 */
describe("EmberOrb", () => {
  afterEach(() => {
    cleanup();
  });

  it("renders at the requested size with a canvas", () => {
    render(<EmberOrb phase="idle" amplitude={0} size={56} />);

    const orb = screen.getByTestId("dictation-ember-orb");
    expect(orb.style.width).toBe("56px");
    expect(orb.style.height).toBe("56px");
    expect(orb.dataset.emberPhase).toBe("idle");

    const canvas = orb.querySelector("canvas");
    expect(canvas).not.toBeNull();
    expect(canvas?.style.width).toBe("56px");
  });

  it("tracks the phase attribute for every phase", () => {
    for (const phase of ["idle", "listening", "processing", "error"] as const) {
      cleanup();
      render(<EmberOrb phase={phase} amplitude={0.4} size={56} />);
      expect(screen.getByTestId("dictation-ember-orb").dataset.emberPhase).toBe(
        phase,
      );
    }
  });

  it("shows the badge dot on error only", () => {
    render(<EmberOrb phase="error" amplitude={0} size={56} />);
    expect(screen.getByTestId("dictation-ember-error-badge")).not.toBeNull();

    cleanup();
    render(<EmberOrb phase="listening" amplitude={0.6} size={56} />);
    expect(screen.queryByTestId("dictation-ember-error-badge")).toBeNull();
  });
});

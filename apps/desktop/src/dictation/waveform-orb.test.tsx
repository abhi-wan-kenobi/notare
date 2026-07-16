import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";

import { WaveformOrb } from "./waveform-orb";

/** The animated stick bars `DancingSticks` renders when amplitude > 0. */
function queryDancingSticks(container: HTMLElement) {
  return container.querySelectorAll(".animate-hypr-dancing-stick");
}

describe("WaveformOrb", () => {
  afterEach(() => {
    cleanup();
  });

  it("renders a round chassis at the requested size", () => {
    render(<WaveformOrb phase="idle" amplitude={0} size={40} />);

    const orb = screen.getByTestId("dictation-waveform-orb");
    expect(orb.style.width).toBe("40px");
    expect(orb.style.height).toBe("40px");
    expect(orb.className).toContain("rounded-full");
  });

  it("shows a flat quiet line while idle", () => {
    const { container } = render(
      <WaveformOrb phase="idle" amplitude={0} size={40} />,
    );

    expect(
      screen.getByTestId("dictation-waveform-orb").dataset.waveformPhase,
    ).toBe("idle");
    expect(queryDancingSticks(container)).toHaveLength(0);
  });

  it("dances with the amplitude while listening", () => {
    const { container } = render(
      <WaveformOrb phase="listening" amplitude={0.6} size={40} />,
    );

    expect(queryDancingSticks(container).length).toBeGreaterThan(0);
  });

  it("keeps the sticks alive during listening pauses (zero amplitude)", () => {
    const { container } = render(
      <WaveformOrb phase="listening" amplitude={0} size={40} />,
    );

    // The listening floor prevents the flat idle line mid-dictation.
    expect(queryDancingSticks(container).length).toBeGreaterThan(0);
  });

  it("pulses with a constant low dance while processing", () => {
    const { container } = render(
      <WaveformOrb phase="processing" amplitude={0} size={40} />,
    );

    const orb = screen.getByTestId("dictation-waveform-orb");
    expect(orb.className).toContain("animate-orb-pulse");
    expect(queryDancingSticks(container).length).toBeGreaterThan(0);
  });

  it("goes flat and shows the badge dot on error", () => {
    const { container } = render(
      <WaveformOrb phase="error" amplitude={0.8} size={40} />,
    );

    expect(queryDancingSticks(container)).toHaveLength(0);
    expect(
      screen.getByTestId("dictation-waveform-error-badge"),
    ).not.toBeNull();
  });

  it("freezes descendant animations under reduced motion via CSS", () => {
    render(<WaveformOrb phase="listening" amplitude={0.6} size={40} />);

    // The static-frame behavior is CSS-only: a motion-reduce variant that
    // kills every descendant animation (jsdom does not apply media queries,
    // so assert the class is wired).
    expect(
      screen.getByTestId("dictation-waveform-orb").className,
    ).toContain("motion-reduce:[&_*]:animate-none");
  });
});

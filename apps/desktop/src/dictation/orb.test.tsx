import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";

import type { DictationPhase } from "@hypr/plugin-dictation";

import { DictationOrb } from "./orb";

describe("DictationOrb compact waveform", () => {
  afterEach(cleanup);

  it.each<DictationPhase>([
    "idle",
    "listening",
    "processing",
    "success",
    "error",
  ])("keeps the same compact shell and mic state during %s", (phase) => {
    render(<DictationOrb phase={phase} amplitude={0.6} />);
    const waveform = screen.getByTestId("dictation-waveform-orb");
    expect(waveform.dataset.waveformPhase).toBe(phase);
    expect(waveform.style.width).toBe("62px");
    expect(waveform.style.height).toBe("30px");
    expect(screen.getByTestId("dictation-orb").dataset.dictationPhase).toBe(
      phase,
    );
    expect(screen.queryByTestId("dictation-orb-processing") !== null).toBe(
      phase === "processing",
    );
    expect(screen.queryByTestId("dictation-orb-success") !== null).toBe(
      phase === "success",
    );
    expect(
      screen.queryByTestId("dictation-waveform-error-badge") !== null,
    ).toBe(phase === "error");
  });

  it("keeps accent colour in the waveform, not the shell or status overlays", () => {
    const { container } = render(<DictationOrb phase="success" />);
    const shell = screen.getByTestId("dictation-waveform-orb");
    expect(shell.getAttribute("style")).toBe("width: 62px; height: 30px;");
    expect(shell.className).not.toMatch(/primary|accent-glow|shadow-glow/);
    expect(screen.getByTestId("dictation-orb-success").className).not.toMatch(
      /emerald|primary|accent/,
    );
    expect(container.querySelector("canvas")).toBeNull();
  });
});

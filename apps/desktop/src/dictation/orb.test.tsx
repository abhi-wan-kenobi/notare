import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";

import { DictationOrb } from "./orb";

describe("DictationOrb", () => {
  afterEach(() => {
    cleanup();
  });

  it("maps idle to the idle orb state", () => {
    render(<DictationOrb phase="idle" />);

    expect(screen.getByTestId("recording-orb").dataset.orbState).toBe("idle");
    expect(screen.getByTestId("dictation-orb").dataset.dictationPhase).toBe(
      "idle",
    );
  });

  it("maps listening to the listening orb state with amplitude", () => {
    render(<DictationOrb phase="listening" amplitude={0.7} />);

    expect(screen.getByTestId("recording-orb").dataset.orbState).toBe(
      "listening",
    );
  });

  it("maps processing to a pulsing idle orb", () => {
    render(<DictationOrb phase="processing" />);

    expect(screen.getByTestId("recording-orb").dataset.orbState).toBe("idle");
    expect(
      screen.getByTestId("dictation-orb").querySelector(".animate-orb-pulse"),
    ).not.toBeNull();
  });

  it("maps error to the error orb state", () => {
    render(<DictationOrb phase="error" />);

    expect(screen.getByTestId("recording-orb").dataset.orbState).toBe("error");
    expect(screen.getByTestId("recording-orb-error-badge")).not.toBeNull();
  });

  it("records the rendered variant for the future style picker", () => {
    render(<DictationOrb phase="idle" variant="cobalt" />);

    expect(screen.getByTestId("dictation-orb").dataset.dictationVariant).toBe(
      "cobalt",
    );
  });
});

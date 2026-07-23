import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";

import type { DictationPhase } from "@hypr/plugin-dictation";

import {
  DictationOrb,
  normalizeOrbVariant,
  ORB_VARIANT_ORDER,
  ORB_VARIANT_REGISTRY,
  orbSizeForVariant,
  orbWindowSizeForVariant,
} from "./orb";

describe("DictationOrb", () => {
  afterEach(() => {
    cleanup();
  });

  it("maps idle to the idle orb state", () => {
    render(<DictationOrb phase="idle" variant="cobalt" />);

    expect(screen.getByTestId("recording-orb").dataset.orbState).toBe("idle");
    expect(screen.getByTestId("dictation-orb").dataset.dictationPhase).toBe(
      "idle",
    );
  });

  it("maps listening to the listening orb state with amplitude", () => {
    render(<DictationOrb phase="listening" amplitude={0.7} variant="cobalt" />);

    expect(screen.getByTestId("recording-orb").dataset.orbState).toBe(
      "listening",
    );
  });

  it("maps processing to a pulsing idle orb", () => {
    render(<DictationOrb phase="processing" variant="cobalt" />);

    expect(screen.getByTestId("recording-orb").dataset.orbState).toBe("idle");
    expect(
      screen.getByTestId("dictation-orb").querySelector(".animate-orb-pulse"),
    ).not.toBeNull();
  });

  it("maps error to the error orb state", () => {
    render(<DictationOrb phase="error" variant="cobalt" />);

    expect(screen.getByTestId("recording-orb").dataset.orbState).toBe("error");
    expect(screen.getByTestId("recording-orb-error-badge")).not.toBeNull();
  });

  it("records the rendered variant for the style picker", () => {
    render(<DictationOrb phase="idle" variant="cobalt" />);

    expect(screen.getByTestId("dictation-orb").dataset.dictationVariant).toBe(
      "cobalt",
    );
  });

  it("renders the particle sphere for the particles variant", () => {
    render(
      <DictationOrb phase="listening" amplitude={0.5} variant="particles" />,
    );

    expect(screen.getByTestId("dictation-orb").dataset.dictationVariant).toBe(
      "particles",
    );
    expect(screen.getByTestId("dictation-particle-orb")).not.toBeNull();
    expect(screen.queryByTestId("recording-orb")).toBeNull();
  });

  it("renders the particle sphere 1.5x bigger than the base size", () => {
    render(<DictationOrb phase="idle" size={40} variant="particles" />);

    const canvas = screen.getByTestId("dictation-particle-orb");
    expect(canvas.style.width).toBe("60px");
    expect(canvas.style.height).toBe("60px");
  });

  it("renders the Pulse waveform for the waveform variant", () => {
    render(
      <DictationOrb phase="listening" amplitude={0.5} variant="waveform" />,
    );

    expect(screen.getByTestId("dictation-orb").dataset.dictationVariant).toBe(
      "waveform",
    );
    expect(screen.getByTestId("dictation-waveform-orb")).not.toBeNull();
    expect(screen.queryByTestId("recording-orb")).toBeNull();
    expect(screen.queryByTestId("dictation-particle-orb")).toBeNull();
  });

  it("renders the Bloom variant", () => {
    render(<DictationOrb phase="listening" amplitude={0.5} variant="bloom" />);

    expect(screen.getByTestId("dictation-orb").dataset.dictationVariant).toBe(
      "bloom",
    );
    expect(screen.getByTestId("dictation-bloom-orb")).not.toBeNull();
    expect(screen.queryByTestId("recording-orb")).toBeNull();
  });

  it("renders the Halo variant", () => {
    render(<DictationOrb phase="listening" amplitude={0.5} variant="halo" />);

    expect(screen.getByTestId("dictation-orb").dataset.dictationVariant).toBe(
      "halo",
    );
    expect(screen.getByTestId("dictation-halo-orb")).not.toBeNull();
    expect(screen.queryByTestId("recording-orb")).toBeNull();
  });

  it("renders the Ember variant", () => {
    render(<DictationOrb phase="listening" amplitude={0.5} variant="ember" />);

    expect(screen.getByTestId("dictation-orb").dataset.dictationVariant).toBe(
      "ember",
    );
    expect(screen.getByTestId("dictation-ember-orb")).not.toBeNull();
    expect(screen.queryByTestId("recording-orb")).toBeNull();
  });

  it("renders the Silk variant", () => {
    render(<DictationOrb phase="listening" amplitude={0.5} variant="silk" />);

    expect(screen.getByTestId("dictation-orb").dataset.dictationVariant).toBe(
      "silk",
    );
    expect(screen.getByTestId("dictation-silk-orb")).not.toBeNull();
    expect(screen.queryByTestId("recording-orb")).toBeNull();
  });

  it("renders the Pip variant", () => {
    render(<DictationOrb phase="listening" amplitude={0.5} variant="pip" />);

    expect(screen.getByTestId("dictation-orb").dataset.dictationVariant).toBe(
      "pip",
    );
    expect(screen.getByTestId("dictation-pip-orb")).not.toBeNull();
    expect(screen.queryByTestId("recording-orb")).toBeNull();
  });
});

describe("new orb variants", () => {
  afterEach(() => {
    cleanup();
  });

  it("renders the Ring variant", () => {
    render(<DictationOrb phase="listening" amplitude={0.5} variant="ring" />);

    expect(screen.getByTestId("dictation-orb").dataset.dictationVariant).toBe(
      "ring",
    );
    expect(screen.getByTestId("dictation-ring-orb")).not.toBeNull();
    expect(screen.queryByTestId("recording-orb")).toBeNull();
  });

  it("renders the Aurora variant", () => {
    render(<DictationOrb phase="idle" variant="aurora" />);

    expect(screen.getByTestId("dictation-orb").dataset.dictationVariant).toBe(
      "aurora",
    );
    expect(screen.getByTestId("dictation-aurora-orb")).not.toBeNull();
  });

  it("renders the Mono variant", () => {
    render(<DictationOrb phase="idle" variant="mono" />);

    expect(screen.getByTestId("dictation-orb").dataset.dictationVariant).toBe(
      "mono",
    );
    expect(screen.getByTestId("dictation-mono-orb")).not.toBeNull();
  });

  it.each<DictationPhase>(["idle", "listening", "processing", "error"])(
    "renders the Cobalt Halo variant in the %s phase",
    (phase) => {
      render(
        <DictationOrb phase={phase} amplitude={0.5} variant="cobalt-halo" />,
      );

      expect(screen.getByTestId("dictation-orb").dataset.dictationVariant).toBe(
        "cobalt-halo",
      );
      expect(screen.getByTestId("dictation-cobalt-halo-orb")).not.toBeNull();
      expect(screen.queryByTestId("recording-orb")).toBeNull();
    },
  );

  it("shows the destructive badge for the Cobalt Halo error phase", () => {
    render(<DictationOrb phase="error" variant="cobalt-halo" />);

    expect(
      screen.getByTestId("dictation-cobalt-halo-error-badge"),
    ).not.toBeNull();
  });

  it("uses Cobalt Halo as the default variant", () => {
    render(<DictationOrb phase="idle" />);

    expect(screen.getByTestId("dictation-orb").dataset.dictationVariant).toBe(
      "cobalt-halo",
    );
    expect(screen.getByTestId("dictation-cobalt-halo-orb")).not.toBeNull();
  });
});

describe("orb variant registry", () => {
  it("lists every variant for the settings picker", () => {
    expect(ORB_VARIANT_ORDER).toEqual([
      "cobalt",
      "particles",
      "waveform",
      "ring",
      "aurora",
      "mono",
      "bloom",
      "halo",
      "ember",
      "silk",
      "pip",
      "cobalt-halo",
    ]);
    for (const variant of ORB_VARIANT_ORDER) {
      const info = ORB_VARIANT_REGISTRY[variant];
      expect(info.component).toBeTypeOf("function");
      expect(info.title).toBeTruthy();
      expect(info.description).toBeTruthy();
    }
  });
});

describe("normalizeOrbVariant", () => {
  it("maps stored strings onto known variants", () => {
    expect(normalizeOrbVariant("particles")).toBe("particles");
    expect(normalizeOrbVariant("waveform")).toBe("waveform");
    expect(normalizeOrbVariant("cobalt")).toBe("cobalt");
    expect(normalizeOrbVariant("ring")).toBe("ring");
    expect(normalizeOrbVariant("aurora")).toBe("aurora");
    expect(normalizeOrbVariant("mono")).toBe("mono");
    expect(normalizeOrbVariant("bloom")).toBe("bloom");
    expect(normalizeOrbVariant("halo")).toBe("halo");
    expect(normalizeOrbVariant("ember")).toBe("ember");
    expect(normalizeOrbVariant("silk")).toBe("silk");
    expect(normalizeOrbVariant("pip")).toBe("pip");
    expect(normalizeOrbVariant("cobalt-halo")).toBe("cobalt-halo");
    expect(normalizeOrbVariant(undefined)).toBe("cobalt-halo");
    expect(normalizeOrbVariant("garbage")).toBe("cobalt-halo");
  });
});

describe("orb variant sizing", () => {
  it("scales the orb 1.5x for particles only", () => {
    expect(orbSizeForVariant("cobalt", 40)).toBe(40);
    expect(orbSizeForVariant("waveform", 40)).toBe(40);
    expect(orbSizeForVariant("ring", 40)).toBe(40);
    expect(orbSizeForVariant("aurora", 40)).toBe(40);
    expect(orbSizeForVariant("mono", 40)).toBe(40);
    expect(orbSizeForVariant("bloom", 40)).toBe(40);
    expect(orbSizeForVariant("halo", 40)).toBe(40);
    expect(orbSizeForVariant("ember", 40)).toBe(40);
    expect(orbSizeForVariant("silk", 40)).toBe(40);
    expect(orbSizeForVariant("pip", 40)).toBe(40);
    expect(orbSizeForVariant("cobalt-halo", 40)).toBe(40);
    expect(orbSizeForVariant("particles", 40)).toBe(60);
    expect(orbSizeForVariant("particles", 28)).toBe(42);
  });

  it("scales the orb window to match (Rust creates it at 70px)", () => {
    expect(orbWindowSizeForVariant("cobalt")).toBe(70);
    expect(orbWindowSizeForVariant("waveform")).toBe(70);
    expect(orbWindowSizeForVariant("particles")).toBe(105);
    expect(orbWindowSizeForVariant("ring")).toBe(70);
    expect(orbWindowSizeForVariant("aurora")).toBe(70);
    expect(orbWindowSizeForVariant("mono")).toBe(70);
    expect(orbWindowSizeForVariant("bloom")).toBe(70);
    expect(orbWindowSizeForVariant("halo")).toBe(70);
    expect(orbWindowSizeForVariant("ember")).toBe(70);
    expect(orbWindowSizeForVariant("silk")).toBe(70);
    expect(orbWindowSizeForVariant("pip")).toBe(70);
    expect(orbWindowSizeForVariant("cobalt-halo")).toBe(70);
  });
});

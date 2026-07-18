import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import {
  afterEach,
  beforeEach,
  describe,
  expect,
  it,
  vi,
} from "vitest";

import type { DictationPhase } from "@hypr/plugin-dictation";

import { DictationOrb, ORB_VARIANT_ORDER } from "./orb";

import { OrbVariantGroup } from "~/settings/dictation";

/**
 * jsdom ships no canvas, so `test-setup.ts` makes every `getContext()` call
 * return `null` and every orb's renderer bails out on that null-context
 * guard - which means the *actual drawing code* (`drawFrame`/`stepState` in
 * each `*-orb.tsx`) has never executed in CI. A throw in there (e.g. a
 * negative radius reaching `ctx.arc`/`ctx.ellipse`/`createRadialGradient`,
 * all of which throw `IndexSizeError` in real browsers/WebView2 on a
 * negative radius) would sail through every existing orb test untouched and
 * only surface at runtime, in the one place that has a real 2D context.
 *
 * This file gives canvases a minimal but *behaviourally faithful* fake
 * `CanvasRenderingContext2D` (faithful specifically on the one thing that
 * throws: negative radii) and a manually-driven `requestAnimationFrame`, so
 * every orb's renderer actually runs, across every phase/amplitude it can be
 * driven with, at the settings-picker's live preview size.
 */

interface RafEntry {
  id: number;
  callback: FrameRequestCallback;
}

function installFakeCanvas() {
  let nextRafId = 1;
  const scheduled: RafEntry[] = [];
  let now = 0;

  const originalRaf = window.requestAnimationFrame;
  const originalCancelRaf = window.cancelAnimationFrame;
  const originalGetContext = HTMLCanvasElement.prototype.getContext;

  window.requestAnimationFrame = vi.fn((callback: FrameRequestCallback) => {
    const id = nextRafId++;
    scheduled.push({ id, callback });
    return id;
  }) as typeof window.requestAnimationFrame;

  window.cancelAnimationFrame = vi.fn((id: number) => {
    const index = scheduled.findIndex((entry) => entry.id === id);
    if (index !== -1) {
      scheduled.splice(index, 1);
    }
  }) as typeof window.cancelAnimationFrame;

  HTMLCanvasElement.prototype.getContext = vi.fn(function (
    this: HTMLCanvasElement,
    contextId: string,
  ) {
    if (contextId !== "2d") {
      return null;
    }
    return createFakeContext2D();
  }) as unknown as HTMLCanvasElement["getContext"];

  /** Run every currently-queued rAF callback once, advancing the fake clock ~16ms. */
  function flushFrame() {
    now += 16;
    const pending = scheduled.splice(0, scheduled.length);
    for (const entry of pending) {
      entry.callback(now);
    }
  }

  function flushFrames(count: number) {
    for (let i = 0; i < count; i += 1) {
      flushFrame();
    }
  }

  function restore() {
    window.requestAnimationFrame = originalRaf;
    window.cancelAnimationFrame = originalCancelRaf;
    HTMLCanvasElement.prototype.getContext = originalGetContext;
  }

  return { flushFrame, flushFrames, restore };
}

/**
 * A gradient stub - real gradients are opaque objects whose only public
 * surface is `addColorStop`, which itself throws on an out-of-range offset
 * or non-finite color; every call site here uses well-formed offsets, so a
 * permissive stub is faithful enough.
 */
function createFakeGradient(): CanvasGradient {
  return {
    addColorStop: (offset: number) => {
      if (!Number.isFinite(offset) || offset < 0 || offset > 1) {
        throw new DOMException(
          `Failed to execute 'addColorStop': The provided value (${offset}) is outside the range [0, 1].`,
          "IndexSizeError",
        );
      }
    },
  } as unknown as CanvasGradient;
}

/**
 * The narrow slice of CanvasRenderingContext2D every orb actually calls
 * (enumerated from `apps/desktop/src/dictation/*-orb.tsx`). Faithful to the
 * spec on the one behaviour that matters for this investigation: `arc`,
 * `ellipse` and `createRadialGradient` throw `IndexSizeError` on a negative
 * radius, exactly like a real browser/WebView2 2D context does.
 */
function createFakeContext2D(): CanvasRenderingContext2D {
  const assertNonNegativeRadius = (label: string, radius: number) => {
    if (Number.isFinite(radius) && radius < 0) {
      throw new DOMException(
        `Failed to execute '${label}' on 'CanvasRenderingContext2D': The radius provided (${radius}) is negative.`,
        "IndexSizeError",
      );
    }
  };

  const ctx: Record<string, unknown> = {
    // State stack (no-ops - nothing here reads state back).
    save: () => {},
    restore: () => {},
    translate: () => {},
    rotate: () => {},
    scale: () => {},
    setTransform: () => {},
    clip: () => {},

    // Style properties: plain read/write, no validation needed.
    fillStyle: "#000",
    strokeStyle: "#000",
    lineWidth: 1,
    lineCap: "butt",
    lineDashOffset: 0,
    globalAlpha: 1,
    globalCompositeOperation: "source-over",
    filter: "none",

    clearRect: () => {},
    fillRect: () => {},
    setLineDash: () => {},

    beginPath: () => {},
    closePath: () => {},
    moveTo: () => {},
    lineTo: () => {},
    bezierCurveTo: () => {},
    quadraticCurveTo: () => {},
    rect: () => {},
    fill: () => {},
    stroke: () => {},
    drawImage: () => {},

    arc: (
      _x: number,
      _y: number,
      radius: number,
      _start: number,
      _end: number,
    ) => {
      assertNonNegativeRadius("arc", radius);
    },
    ellipse: (
      _x: number,
      _y: number,
      radiusX: number,
      radiusY: number,
      _rotation: number,
      _start: number,
      _end: number,
    ) => {
      assertNonNegativeRadius("ellipse", radiusX);
      assertNonNegativeRadius("ellipse", radiusY);
    },
    createRadialGradient: (
      _x0: number,
      _y0: number,
      r0: number,
      _x1: number,
      _y1: number,
      r1: number,
    ) => {
      assertNonNegativeRadius("createRadialGradient", r0);
      assertNonNegativeRadius("createRadialGradient", r1);
      return createFakeGradient();
    },
    createLinearGradient: () => createFakeGradient(),
    createConicGradient: () => createFakeGradient(),
  };

  return ctx as unknown as CanvasRenderingContext2D;
}

/** The picker's live preview size (`ORB_PREVIEW_BASE_SIZE` in settings/dictation). */
const PREVIEW_SIZE = 64;
const PHASES: DictationPhase[] = ["idle", "listening", "processing", "error"];
const AMPLITUDES = [0, 0.15, 0.5, 0.85, 1];

describe("orb canvas renderers (real-context safety)", () => {
  let fake: ReturnType<typeof installFakeCanvas>;

  beforeEach(() => {
    fake = installFakeCanvas();
  });

  afterEach(() => {
    cleanup();
    fake.restore();
  });

  it.each(ORB_VARIANT_ORDER)(
    "draws every phase/amplitude combination without throwing: %s",
    (variant) => {
      for (const phase of PHASES) {
        for (const amplitude of AMPLITUDES) {
          expect(() => {
            const { unmount } = render(
              <DictationOrb
                phase={phase}
                amplitude={amplitude}
                size={PREVIEW_SIZE}
                variant={variant}
              />,
            );
            // A few seconds of frames: long enough to cross the periodic
            // behaviours (Pip's ~4.2s blink, Ember's ~4.9s sweep, Halo's
            // envelope) at least once.
            fake.flushFrames(180);
            unmount();
          }).not.toThrow();
        }
      }
    },
  );

  it("keeps drawing across a full phase transition without unmounting", () => {
    for (const variant of ORB_VARIANT_ORDER) {
      expect(() => {
        const { rerender, unmount } = render(
          <DictationOrb
            phase="idle"
            amplitude={0}
            size={PREVIEW_SIZE}
            variant={variant}
          />,
        );
        fake.flushFrames(30);

        for (const amplitude of [0.2, 0.6, 0.9, 0.3, 0]) {
          rerender(
            <DictationOrb
              phase="listening"
              amplitude={amplitude}
              size={PREVIEW_SIZE}
              variant={variant}
            />,
          );
          fake.flushFrames(10);
        }

        rerender(
          <DictationOrb
            phase="processing"
            amplitude={0}
            size={PREVIEW_SIZE}
            variant={variant}
          />,
        );
        fake.flushFrames(30);

        rerender(
          <DictationOrb
            phase="error"
            amplitude={0}
            size={PREVIEW_SIZE}
            variant={variant}
          />,
        );
        fake.flushFrames(30);

        unmount();
      }, `variant "${variant}" threw across a phase transition`).not.toThrow();
    }
  });
});

describe("OrbVariantGroup (real-context safety)", () => {
  let fake: ReturnType<typeof installFakeCanvas>;

  beforeEach(() => {
    fake = installFakeCanvas();
  });

  afterEach(() => {
    cleanup();
    fake.restore();
  });

  it("clicks through every variant card in the picker without throwing or blanking", () => {
    let value: string = "cobalt";
    const onChange = vi.fn((next: string) => {
      value = next;
    });

    const { rerender } = render(
      <OrbVariantGroup value={value as never} onChange={onChange} />,
    );
    fake.flushFrames(10);

    expect(() => {
      for (const variant of ORB_VARIANT_ORDER) {
        const card = screen.getByTestId(`orb-preview-card-${variant}`);
        fireEvent.mouseEnter(card);
        fake.flushFrames(3);

        const radio = card.querySelector('input[type="radio"]');
        if (radio) {
          fireEvent.click(radio);
        }
        fake.flushFrames(3);

        rerender(
          <OrbVariantGroup value={variant as never} onChange={onChange} />,
        );
        fake.flushFrames(5);

        fireEvent.mouseLeave(card);
        fake.flushFrames(3);
      }
    }).not.toThrow();

    // The whole grid must still be present - a swallowed render error
    // (caught only by the app's root boundary, well above this component)
    // would otherwise leave this subtree empty.
    for (const variant of ORB_VARIANT_ORDER) {
      expect(screen.getByTestId(`orb-preview-card-${variant}`)).toBeTruthy();
    }
  });

  it("rapidly re-selects variants back and forth without throwing", () => {
    const onChange = vi.fn();
    const { rerender } = render(
      <OrbVariantGroup value={"cobalt" as never} onChange={onChange} />,
    );
    fake.flushFrames(5);

    const sequence = [
      "particles",
      "cobalt",
      "pip",
      "halo",
      "cobalt",
      "silk",
      "bloom",
      "ember",
      "cobalt",
    ] as const;

    expect(() => {
      for (const variant of sequence) {
        rerender(
          <OrbVariantGroup value={variant as never} onChange={onChange} />,
        );
        // No frames flushed between selections on purpose - this simulates
        // a user clicking through cards faster than a frame boundary.
      }
      fake.flushFrames(60);
    }).not.toThrow();
  });
});

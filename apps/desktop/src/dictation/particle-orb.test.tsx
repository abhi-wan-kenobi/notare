import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";

import {
  createParticles,
  PARTICLE_ORB_BASE_SIZE,
  PARTICLE_ORB_PARTICLE_COUNT,
  particleCountForSize,
  ParticleOrb,
} from "./particle-orb";

describe("createParticles", () => {
  it("creates the requested number of particles", () => {
    expect(createParticles(PARTICLE_ORB_PARTICLE_COUNT)).toHaveLength(
      PARTICLE_ORB_PARTICLE_COUNT,
    );
  });

  it("distributes radii across shell, inner fill and halo", () => {
    // Deterministic LCG so the distribution assertion cannot flake.
    let state = 42;
    const rng = () => {
      state = (state * 1664525 + 1013904223) % 4294967296;
      return state / 4294967296;
    };

    const particles = createParticles(2000, rng);
    let shell = 0;
    let inner = 0;
    let halo = 0;

    for (const p of particles) {
      const r = Math.hypot(p.x, p.y, p.z);
      expect(r).toBeGreaterThanOrEqual(0.4);
      expect(r).toBeLessThanOrEqual(1.6);
      if (r >= 0.95 && r <= 1.1) shell++;
      else if (r < 0.95) inner++;
      else halo++;

      expect(p.seed).toBeGreaterThanOrEqual(0);
      expect(p.seed).toBeLessThanOrEqual(1);
      expect(p.size).toBeGreaterThanOrEqual(0.3);
      expect(p.size).toBeLessThanOrEqual(1);
    }

    // Reference distribution: ~60% shell, ~30% inner, ~10% halo.
    expect(shell / particles.length).toBeGreaterThan(0.5);
    expect(inner / particles.length).toBeGreaterThan(0.2);
    expect(halo / particles.length).toBeGreaterThan(0.05);
  });
});

describe("particleCountForSize", () => {
  it("keeps the reference count at the base size", () => {
    expect(particleCountForSize(PARTICLE_ORB_BASE_SIZE)).toBe(
      PARTICLE_ORB_PARTICLE_COUNT,
    );
  });

  it("scales the count linearly with the diameter", () => {
    // The 1.5x dictation orb (40 -> 60px).
    expect(particleCountForSize(60)).toBe(510);
    // Settings preview (28px) and its 1.5x variant (42px).
    expect(particleCountForSize(28)).toBe(238);
    expect(particleCountForSize(42)).toBe(357);
  });
});

describe("ParticleOrb", () => {
  afterEach(() => {
    cleanup();
  });

  it("renders a decorative canvas without crashing in jsdom", () => {
    // jsdom has no 2D context; the component must tolerate getContext()
    // returning null (and stay quiet - the canvas is aria-hidden).
    render(<ParticleOrb phase="listening" amplitude={0.5} size={40} />);

    const canvas = screen.getByTestId("dictation-particle-orb");
    expect(canvas.tagName).toBe("CANVAS");
    expect(canvas.getAttribute("aria-hidden")).not.toBeNull();
    expect(canvas.style.width).toBe("40px");
    expect(canvas.style.height).toBe("40px");
  });

  it("shows the shared destructive badge dot only in the error phase", () => {
    // Parity with the other alive variants: at 32px the desaturation-only
    // error treatment reads as a dim idle, so pair it with the badge dot (#34).
    const { rerender } = render(
      <ParticleOrb phase="listening" amplitude={0.5} size={32} />,
    );
    expect(
      screen.queryByTestId("dictation-particle-error-badge"),
    ).toBeNull();

    rerender(<ParticleOrb phase="error" amplitude={0} size={32} />);
    expect(
      screen.getByTestId("dictation-particle-error-badge"),
    ).not.toBeNull();
  });
});

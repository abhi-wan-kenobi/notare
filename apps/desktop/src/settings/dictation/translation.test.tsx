import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { normalizeTranslationTarget, TranslationSettings } from "./translation";

describe("normalizeTranslationTarget", () => {
  it("keeps a known target code", () => {
    expect(normalizeTranslationTarget("hi")).toBe("hi");
  });

  it("falls back to English for an unknown or missing code", () => {
    expect(normalizeTranslationTarget("xx")).toBe("en");
    expect(normalizeTranslationTarget(undefined)).toBe("en");
  });
});

describe("TranslationSettings", () => {
  afterEach(() => {
    cleanup();
  });

  it("toggles the switch and reports the new checked state", () => {
    const onToggle = vi.fn();
    render(
      <TranslationSettings
        enabled={false}
        target="en"
        modelAvailable
        onToggle={onToggle}
        onTargetChange={vi.fn()}
      />,
    );

    const toggle = screen.getByRole("switch", {
      name: /Translate dictation/,
    });
    expect((toggle as HTMLButtonElement).getAttribute("aria-checked")).toBe(
      "false",
    );

    fireEvent.click(toggle);
    expect(onToggle).toHaveBeenCalledWith(true);
  });

  it("mentions the current target language in the description", () => {
    render(
      <TranslationSettings
        enabled={false}
        target="hi"
        modelAvailable
        onToggle={vi.fn()}
        onTargetChange={vi.fn()}
      />,
    );

    expect(screen.getByText(/insert the text in Hindi/)).toBeTruthy();
  });

  it("hides the target-language picker while disabled, shows it once enabled", () => {
    const { rerender } = render(
      <TranslationSettings
        enabled={false}
        target="en"
        modelAvailable
        onToggle={vi.fn()}
        onTargetChange={vi.fn()}
      />,
    );
    expect(screen.queryByRole("radiogroup")).toBeNull();

    rerender(
      <TranslationSettings
        enabled
        target="en"
        modelAvailable
        onToggle={vi.fn()}
        onTargetChange={vi.fn()}
      />,
    );
    expect(screen.getByRole("radiogroup")).toBeTruthy();
  });

  it("offers the curated language list and reports the chosen target code", () => {
    const onTargetChange = vi.fn();
    render(
      <TranslationSettings
        enabled
        target="en"
        modelAvailable
        onToggle={vi.fn()}
        onTargetChange={onTargetChange}
      />,
    );

    const english = screen.getByRole("radio", { name: "English" });
    expect((english as HTMLInputElement).checked).toBe(true);

    fireEvent.click(screen.getByRole("radio", { name: "Hindi" }));
    expect(onTargetChange).toHaveBeenCalledWith("hi");
  });

  it("shows a helper about the missing model when unavailable, and hides it once a model is ready", () => {
    const { rerender } = render(
      <TranslationSettings
        enabled={false}
        target="en"
        modelAvailable={false}
        onToggle={vi.fn()}
        onTargetChange={vi.fn()}
      />,
    );
    expect(
      screen.getByText(/AI cleanup isn't using a language model/),
    ).toBeTruthy();

    rerender(
      <TranslationSettings
        enabled={false}
        target="en"
        modelAvailable
        onToggle={vi.fn()}
        onTargetChange={vi.fn()}
      />,
    );
    expect(
      screen.queryByText(/AI cleanup isn't using a language model/),
    ).toBeNull();
  });

  it("keeps the toggle enabled (not hard-disabled) even when no model is available", () => {
    render(
      <TranslationSettings
        enabled={false}
        target="en"
        modelAvailable={false}
        onToggle={vi.fn()}
        onTargetChange={vi.fn()}
      />,
    );

    const toggle = screen.getByRole("switch", {
      name: /Translate dictation/,
    });
    expect((toggle as HTMLButtonElement).disabled).toBeFalsy();
  });
});

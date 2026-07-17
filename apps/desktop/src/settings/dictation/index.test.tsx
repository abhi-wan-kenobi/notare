import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { DictationHistoryEntry } from "~/dictation/history";
import { ORB_VARIANT_ORDER } from "~/dictation/orb";

import {
  CleanupGroup,
  DictationHistoryList,
  OrbVariantGroup,
  OutputModeGroup,
} from "./index";

describe("OutputModeGroup", () => {
  afterEach(() => {
    cleanup();
  });

  it("selects the current mode and emits the new one", () => {
    const onChange = vi.fn();
    render(<OutputModeGroup value="type" onChange={onChange} />);

    const type = screen.getByRole("radio", { name: /Type as you speak/ });
    const batch = screen.getByRole("radio", {
      name: /Collect and deliver when you stop/,
    });
    expect((type as HTMLInputElement).checked).toBe(true);
    expect((batch as HTMLInputElement).checked).toBe(false);

    fireEvent.click(batch);
    expect(onChange).toHaveBeenCalledWith("batch");
  });
});

describe("CleanupGroup", () => {
  afterEach(() => {
    cleanup();
  });

  it("offers none, basic and AI cleanup", () => {
    const onChange = vi.fn();
    render(<CleanupGroup value="basic" onChange={onChange} />);

    expect(
      (screen.getByRole("radio", { name: /Basic/ }) as HTMLInputElement)
        .checked,
    ).toBe(true);

    fireEvent.click(screen.getByRole("radio", { name: /AI cleanup/ }));
    expect(onChange).toHaveBeenCalledWith("llm");

    fireEvent.click(screen.getByRole("radio", { name: /None/ }));
    expect(onChange).toHaveBeenCalledWith("none");
  });
});

describe("OrbVariantGroup", () => {
  afterEach(() => {
    cleanup();
  });

  it("renders one preview card per registry variant in a radiogroup", () => {
    render(<OrbVariantGroup value="cobalt" onChange={vi.fn()} />);

    expect(screen.getByRole("radiogroup")).toBeTruthy();
    for (const variant of ORB_VARIANT_ORDER) {
      expect(screen.getByTestId(`orb-preview-card-${variant}`)).toBeTruthy();
    }
    expect(screen.getAllByRole("radio")).toHaveLength(
      ORB_VARIANT_ORDER.length,
    );
  });

  it("switches between the cobalt, particle and Pulse orbs", () => {
    const onChange = vi.fn();
    render(<OrbVariantGroup value="cobalt" onChange={onChange} />);

    expect(
      (screen.getByRole("radio", { name: /Cobalt/ }) as HTMLInputElement)
        .checked,
    ).toBe(true);

    fireEvent.click(screen.getByRole("radio", { name: /Particles/ }));
    expect(onChange).toHaveBeenCalledWith("particles");

    fireEvent.click(screen.getByRole("radio", { name: /Pulse/ }));
    expect(onChange).toHaveBeenCalledWith("waveform");
  });

  it("previews every variant live, particles proportionally bigger", () => {
    render(<OrbVariantGroup value="cobalt" onChange={vi.fn()} />);

    expect(screen.getByTestId("recording-orb")).not.toBeNull();
    expect(screen.getByTestId("dictation-waveform-orb")).not.toBeNull();
    expect(screen.getByTestId("dictation-ring-orb")).not.toBeNull();
    expect(screen.getByTestId("dictation-aurora-orb")).not.toBeNull();
    expect(screen.getByTestId("dictation-mono-orb")).not.toBeNull();

    // The particle preview reflects the 1.5x scale (64 -> 96px).
    const particleCanvas = screen.getByTestId("dictation-particle-orb");
    expect(particleCanvas.style.width).toBe("96px");
    expect(particleCanvas.style.height).toBe("96px");
  });

  it("runs the selected card live and the rest idle", () => {
    render(<OrbVariantGroup value="waveform" onChange={vi.fn()} />);

    const selected = screen.getByTestId("orb-preview-card-waveform");
    expect(selected.dataset.selected).toBe("true");
    expect(
      selected.querySelector('[data-dictation-phase="listening"]'),
    ).not.toBeNull();

    const idle = screen.getByTestId("orb-preview-card-cobalt");
    expect(idle.dataset.selected).toBeUndefined();
    expect(idle.querySelector('[data-dictation-phase="idle"]')).not.toBeNull();
  });

  it("wakes a card to listening on hover", () => {
    render(<OrbVariantGroup value="cobalt" onChange={vi.fn()} />);

    const card = screen.getByTestId("orb-preview-card-mono");
    expect(card.querySelector('[data-dictation-phase="idle"]')).not.toBeNull();

    fireEvent.mouseEnter(card);
    expect(
      card.querySelector('[data-dictation-phase="listening"]'),
    ).not.toBeNull();

    fireEvent.mouseLeave(card);
    expect(card.querySelector('[data-dictation-phase="idle"]')).not.toBeNull();
  });
});

describe("DictationHistoryList", () => {
  // Pin the clock so the relative-timestamp assertions are deterministic and
  // don't flake under CI load (the entries use absolute offsets from `now`).
  const NOW = new Date("2026-07-17T12:00:00.000Z").getTime();

  beforeEach(() => {
    vi.useFakeTimers();
    vi.setSystemTime(NOW);
  });
  afterEach(() => {
    vi.useRealTimers();
    cleanup();
  });

  const entries: DictationHistoryEntry[] = [
    {
      id: "one",
      text: "First dictation",
      mode: "type",
      cleaned: true,
      createdAt: new Date(NOW - 30_000).toISOString(),
    },
    {
      id: "two",
      text: "Second dictation",
      mode: "batch",
      cleaned: false,
      createdAt: new Date(NOW - 90_000).toISOString(),
    },
  ];

  it("shows an empty state without entries", () => {
    render(
      <DictationHistoryList entries={[]} onCopy={vi.fn()} onDelete={vi.fn()} />,
    );

    expect(screen.queryByTestId("dictation-history-list")).toBeNull();
    expect(screen.getByText(/Nothing here yet/)).not.toBeNull();
  });

  it("copies an entry on click and deletes via the row button", () => {
    const onCopy = vi.fn();
    const onDelete = vi.fn();
    render(
      <DictationHistoryList
        entries={entries}
        onCopy={onCopy}
        onDelete={onDelete}
      />,
    );

    fireEvent.click(screen.getByText("First dictation"));
    expect(onCopy).toHaveBeenCalledWith(entries[0]);

    const deleteButtons = screen.getAllByRole("button", {
      name: "Delete history entry",
    });
    expect(deleteButtons).toHaveLength(2);
    fireEvent.click(deleteButtons[1]);
    expect(onDelete).toHaveBeenCalledWith(entries[1]);
  });

  it("shows a relative timestamp for each entry", () => {
    render(
      <DictationHistoryList
        entries={entries}
        onCopy={vi.fn()}
        onDelete={vi.fn()}
      />,
    );

    expect(screen.getAllByText(/ago$/)).toHaveLength(2);
  });
});

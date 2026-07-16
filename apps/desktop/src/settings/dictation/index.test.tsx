import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { DictationHistoryEntry } from "~/dictation/history";

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

  it("switches between the cobalt and particle orbs", () => {
    const onChange = vi.fn();
    render(<OrbVariantGroup value="cobalt" onChange={onChange} />);

    expect(
      (screen.getByRole("radio", { name: /Cobalt/ }) as HTMLInputElement)
        .checked,
    ).toBe(true);

    fireEvent.click(screen.getByRole("radio", { name: /Particles/ }));
    expect(onChange).toHaveBeenCalledWith("particles");
  });
});

describe("DictationHistoryList", () => {
  afterEach(() => {
    cleanup();
  });

  const entries: DictationHistoryEntry[] = [
    {
      id: "one",
      text: "First dictation",
      mode: "type",
      cleaned: true,
      createdAt: new Date().toISOString(),
    },
    {
      id: "two",
      text: "Second dictation",
      mode: "batch",
      cleaned: false,
      createdAt: new Date(Date.now() - 60_000).toISOString(),
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

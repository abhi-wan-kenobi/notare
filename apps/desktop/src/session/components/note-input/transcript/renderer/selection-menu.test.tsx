import { act, cleanup, render, screen } from "@testing-library/react";
import { useRef } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { SelectionMenu } from "./selection-menu";

function Harness() {
  const containerRef = useRef<HTMLDivElement>(null);
  return (
    <div ref={containerRef}>
      <p data-testid="text">hello world</p>
      <SelectionMenu containerRef={containerRef} />
    </div>
  );
}

function changeSelection(update: (selection: Selection) => void) {
  act(() => {
    update(window.getSelection()!);
    document.dispatchEvent(new Event("selectionchange"));
  });
}

describe("SelectionMenu", () => {
  beforeEach(() => {
    // jsdom has no layout, so Range has no geometry methods.
    Range.prototype.getClientRects = vi.fn(
      () => [],
    ) as unknown as Range["getClientRects"];
    Range.prototype.getBoundingClientRect = vi.fn(() => new DOMRect());
  });

  afterEach(() => {
    window.getSelection()?.removeAllRanges();
    cleanup();
  });

  it("hides the menu when the selection collapses", () => {
    render(<Harness />);
    const text = screen.getByTestId("text");

    changeSelection((selection) => selection.selectAllChildren(text));
    expect(screen.getByRole("button", { name: "Copy" })).not.toBeNull();

    changeSelection((selection) => selection.collapseToStart());
    expect(screen.queryByRole("button", { name: "Copy" })).toBeNull();
  });

  it("hides the menu when the selection is removed", () => {
    render(<Harness />);
    const text = screen.getByTestId("text");

    changeSelection((selection) => selection.selectAllChildren(text));
    expect(screen.getByRole("button", { name: "Copy" })).not.toBeNull();

    changeSelection((selection) => selection.removeAllRanges());
    expect(screen.queryByRole("button", { name: "Copy" })).toBeNull();
  });
});

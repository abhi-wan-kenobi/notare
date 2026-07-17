import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

type ParseResult =
  | { status: "ok"; data: null }
  | { status: "error"; error: string };

const mocks = vi.hoisted(() => ({
  parseGlobalHotkey: vi.fn<() => Promise<ParseResult>>(async () => ({
    status: "ok",
    data: null,
  })),
}));

vi.mock("@hypr/plugin-shortcut", () => ({
  commands: {
    parseGlobalHotkey: mocks.parseGlobalHotkey,
  },
}));

import { ShortcutRecorderRow } from "./shortcut-recorder";

const DEFAULT = "ctrl+alt+space";

function renderRow({
  value = DEFAULT,
  onCommit = vi.fn(),
}: { value?: string; onCommit?: (next: string) => void } = {}) {
  render(
    <ShortcutRecorderRow
      value={value}
      defaultValue={DEFAULT}
      onCommit={onCommit}
    />,
  );
  return { onCommit };
}

function recorder() {
  return screen.getByTestId("shortcut-recorder");
}

describe("ShortcutRecorderRow", () => {
  beforeEach(() => {
    mocks.parseGlobalHotkey.mockClear();
    mocks.parseGlobalHotkey.mockResolvedValue({ status: "ok", data: null });
  });

  afterEach(() => {
    cleanup();
  });

  it("shows the current combo as keycap chips", () => {
    renderRow();

    const chips = recorder().querySelectorAll("kbd");
    expect(Array.from(chips).map((chip) => chip.textContent)).toEqual([
      "Ctrl",
      "Alt",
      "Space",
    ]);
  });

  it("arms on click and shows the press prompt", () => {
    renderRow();

    fireEvent.click(recorder());

    expect(recorder().dataset.recording).toBe("true");
    expect(screen.getByText("Press shortcut…")).toBeTruthy();
  });

  it("captures a combo, validates it and commits", async () => {
    const { onCommit } = renderRow();

    fireEvent.click(recorder());
    fireEvent.keyDown(recorder(), {
      key: "d",
      code: "KeyD",
      ctrlKey: true,
      shiftKey: true,
    });

    await waitFor(() =>
      expect(mocks.parseGlobalHotkey).toHaveBeenCalledWith("ctrl+shift+d"),
    );
    await waitFor(() => expect(onCommit).toHaveBeenCalledWith("ctrl+shift+d"));
    expect(recorder().dataset.recording).toBeUndefined();
  });

  it("previews held modifiers as chips while recording", () => {
    renderRow();

    fireEvent.click(recorder());
    fireEvent.keyDown(recorder(), {
      key: "Control",
      code: "ControlLeft",
      ctrlKey: true,
    });
    fireEvent.keyDown(recorder(), {
      key: "Alt",
      code: "AltLeft",
      ctrlKey: true,
      altKey: true,
    });

    const chips = recorder().querySelectorAll("kbd");
    expect(Array.from(chips).map((chip) => chip.textContent)).toEqual([
      "Ctrl",
      "Alt",
    ]);
  });

  it("cancels on Escape and keeps the previous value", () => {
    const { onCommit } = renderRow();

    fireEvent.click(recorder());
    fireEvent.keyDown(recorder(), { key: "Escape", code: "Escape" });

    expect(onCommit).not.toHaveBeenCalled();
    expect(recorder().dataset.recording).toBeUndefined();
    const chips = recorder().querySelectorAll("kbd");
    expect(chips).toHaveLength(3);
  });

  it("explains a missing modifier inline and keeps recording", () => {
    const { onCommit } = renderRow();

    fireEvent.click(recorder());
    fireEvent.keyDown(recorder(), { key: "a", code: "KeyA" });

    expect(screen.getByTestId("shortcut-recorder-error")).toBeTruthy();
    expect(recorder().dataset.recording).toBe("true");
    expect(onCommit).not.toHaveBeenCalled();
  });

  it("surfaces a parser rejection inline without committing", async () => {
    mocks.parseGlobalHotkey.mockResolvedValue({
      status: "error",
      error: "invalid shortcut",
    });
    const warnSpy = vi.spyOn(console, "warn").mockImplementation(() => {});
    const { onCommit } = renderRow();

    try {
      fireEvent.click(recorder());
      fireEvent.keyDown(recorder(), { key: "d", code: "KeyD", ctrlKey: true });

      await waitFor(() =>
        expect(screen.getByTestId("shortcut-recorder-error")).toBeTruthy(),
      );
      expect(onCommit).not.toHaveBeenCalled();
    } finally {
      warnSpy.mockRestore();
    }
  });

  it("cancels when the recorder loses focus", () => {
    renderRow();

    fireEvent.click(recorder());
    fireEvent.blur(recorder());

    expect(recorder().dataset.recording).toBeUndefined();
  });

  it("offers reset-to-default only when off the default", () => {
    renderRow();
    expect(
      screen.queryByRole("button", { name: "Reset to the default shortcut" }),
    ).toBeNull();
    cleanup();

    const onCommit = vi.fn();
    renderRow({ value: "ctrl+shift+d", onCommit });
    fireEvent.click(
      screen.getByRole("button", { name: "Reset to the default shortcut" }),
    );
    expect(onCommit).toHaveBeenCalledWith(DEFAULT);
  });
});

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
  // Windows/Linux by default so the existing spelled-out-modifier tests below
  // stay unchanged; macOS-specific tests flip this per-case.
  platform: "windows" as string,
}));

vi.mock("@hypr/plugin-shortcut", () => ({
  commands: {
    parseGlobalHotkey: mocks.parseGlobalHotkey,
  },
}));

vi.mock("@tauri-apps/plugin-os", () => ({
  platform: () => mocks.platform,
}));

import { ShortcutRecorderRow } from "./shortcut-recorder";

const DEFAULT = "ctrl+alt+space";

function renderRow({
  value = DEFAULT,
  onCommit = vi.fn(),
  title,
  description,
  conflictValue,
  conflictMessage,
}: {
  value?: string;
  onCommit?: (next: string) => void;
  title?: React.ReactNode;
  description?: React.ReactNode;
  conflictValue?: string;
  conflictMessage?: string;
} = {}) {
  render(
    <ShortcutRecorderRow
      value={value}
      defaultValue={DEFAULT}
      onCommit={onCommit}
      title={title}
      description={description}
      conflictValue={conflictValue}
      conflictMessage={conflictMessage}
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
    mocks.platform = "windows";
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

  // macOS-specific coverage: (a) the chips must render Mac keyboard-symbol
  // glyphs instead of spelled-out modifier names, and (b) WebKit does not
  // move DOM focus to a <button> on click by default (only Chromium/Firefox
  // do that automatically), so the recorder must explicitly focus itself
  // when armed - otherwise every keydown while "recording" is silently lost
  // because it never reaches the button's handlers, which is exactly the
  // "pressing a combo leaves the field blank" bug on macOS.

  it("renders mac-style modifier glyphs on macOS instead of spelled-out names", () => {
    mocks.platform = "macos";
    renderRow();

    const chips = recorder().querySelectorAll("kbd");
    expect(Array.from(chips).map((chip) => chip.textContent)).toEqual([
      "⌃",
      "⌥",
      "Space",
    ]);
  });

  it("keeps spelled-out modifiers on Windows/Linux (unchanged)", () => {
    mocks.platform = "windows";
    renderRow();

    const chips = recorder().querySelectorAll("kbd");
    expect(Array.from(chips).map((chip) => chip.textContent)).toEqual([
      "Ctrl",
      "Alt",
      "Space",
    ]);
  });

  it("explicitly focuses the recorder button when armed, for WebKit/macOS", () => {
    // jsdom's own click-focusing behavior can't be relied on to prove this -
    // it does not reproduce WebKit's "buttons aren't click-focusable by
    // default" quirk. Spy on the imperative call our fix adds instead.
    const focusSpy = vi.spyOn(HTMLButtonElement.prototype, "focus");
    try {
      renderRow();
      fireEvent.click(recorder());
      expect(focusSpy).toHaveBeenCalledTimes(1);
    } finally {
      focusSpy.mockRestore();
    }
  });

  it("captures a mac Option-modified combo by physical code, even though event.key is garbled", async () => {
    // WebKit/macOS: holding Option remaps `key` to a special character (e.g.
    // Option+D -> "∂") while `code` stays the physical key. The accelerator
    // must resolve from `code`, not the Option-mangled `key`.
    mocks.platform = "macos";
    const { onCommit } = renderRow();

    fireEvent.click(recorder());
    fireEvent.keyDown(recorder(), {
      key: "∂",
      code: "KeyD",
      altKey: true,
    });

    await waitFor(() =>
      expect(mocks.parseGlobalHotkey).toHaveBeenCalledWith("alt+d"),
    );
    await waitFor(() => expect(onCommit).toHaveBeenCalledWith("alt+d"));
  });

  it("captures a Cmd (metaKey) combo on macOS as super+key", async () => {
    mocks.platform = "macos";
    const { onCommit } = renderRow();

    fireEvent.click(recorder());
    fireEvent.keyDown(recorder(), {
      key: "k",
      code: "KeyK",
      metaKey: true,
    });

    await waitFor(() => expect(onCommit).toHaveBeenCalledWith("super+k"));
  });

  // Generalized for reuse by the paste-last-dictation row (Lane A1): a
  // custom title/description overrides the default "Toggle shortcut" copy,
  // and the default copy is untouched when they're omitted (the dictation
  // toggle row's existing call site).
  it("uses the default toggle-shortcut copy when no title/description is given", () => {
    renderRow();

    expect(screen.getByText("Toggle shortcut")).toBeTruthy();
    expect(screen.getByText(/starts or stops dictation/)).toBeTruthy();
  });

  it("renders a custom title and description when provided", () => {
    renderRow({
      title: "Paste last dictation",
      description: "Re-pastes your most recent dictation.",
    });

    expect(screen.getByText("Paste last dictation")).toBeTruthy();
    expect(
      screen.getByText("Re-pastes your most recent dictation."),
    ).toBeTruthy();
    expect(screen.queryByText("Toggle shortcut")).toBeNull();
  });

  it("rejects a candidate that conflicts with another recorder's binding", async () => {
    const { onCommit } = renderRow({
      conflictValue: "ctrl+shift+d",
      conflictMessage: "Already used by the other shortcut.",
    });

    fireEvent.click(recorder());
    fireEvent.keyDown(recorder(), {
      key: "d",
      code: "KeyD",
      ctrlKey: true,
      shiftKey: true,
    });

    expect(screen.getByTestId("shortcut-recorder-error").textContent).toBe(
      "Already used by the other shortcut.",
    );
    expect(onCommit).not.toHaveBeenCalled();
    expect(mocks.parseGlobalHotkey).not.toHaveBeenCalled();
  });

  it("does not conflict-check the recorder's own unchanged value", async () => {
    // `value === conflictValue` happens naturally when both rows are unset
    // ("" default) - re-recording back to the SAME combo the row already
    // has must hit the existing value===candidate no-op, not the conflict
    // path, so the user isn't blocked from "re-confirming" their own combo.
    const { onCommit } = renderRow({ value: DEFAULT, conflictValue: DEFAULT });

    fireEvent.click(recorder());
    fireEvent.keyDown(recorder(), {
      key: "space",
      code: "Space",
      ctrlKey: true,
      altKey: true,
    });

    await waitFor(() => expect(recorder().dataset.recording).toBeUndefined());
    expect(onCommit).not.toHaveBeenCalled();
    expect(mocks.parseGlobalHotkey).not.toHaveBeenCalled();
  });
});

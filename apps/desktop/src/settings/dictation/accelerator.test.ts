import { describe, expect, it } from "vitest";

import {
  acceleratorFromKeydown,
  acceleratorParts,
  heldModifiers,
  keyTokenFromCode,
  type RecorderKeyEvent,
} from "./accelerator";

function event(overrides: Partial<RecorderKeyEvent>): RecorderKeyEvent {
  return {
    key: "",
    code: "",
    ctrlKey: false,
    altKey: false,
    shiftKey: false,
    metaKey: false,
    ...overrides,
  };
}

describe("acceleratorFromKeydown", () => {
  it("commits the default-style combo lowercase", () => {
    expect(
      acceleratorFromKeydown(
        event({ key: " ", code: "Space", ctrlKey: true, altKey: true }),
      ),
    ).toEqual({ kind: "commit", accelerator: "ctrl+alt+space" });
  });

  it("orders modifiers canonically regardless of press order", () => {
    // All four held: the event flags carry no order, the output must.
    expect(
      acceleratorFromKeydown(
        event({
          key: "k",
          code: "KeyK",
          metaKey: true,
          shiftKey: true,
          ctrlKey: true,
          altKey: true,
        }),
      ),
    ).toEqual({ kind: "commit", accelerator: "ctrl+alt+shift+super+k" });
  });

  it("lowercases letters even when shift produces an uppercase key", () => {
    expect(
      acceleratorFromKeydown(
        event({ key: "A", code: "KeyA", ctrlKey: true, shiftKey: true }),
      ),
    ).toEqual({ kind: "commit", accelerator: "ctrl+shift+a" });
  });

  it("maps the meta key to super", () => {
    expect(
      acceleratorFromKeydown(event({ key: "d", code: "KeyD", metaKey: true })),
    ).toEqual({ kind: "commit", accelerator: "super+d" });
  });

  it("commits function keys", () => {
    expect(
      acceleratorFromKeydown(event({ key: "F5", code: "F5", ctrlKey: true })),
    ).toEqual({ kind: "commit", accelerator: "ctrl+f5" });
    expect(
      acceleratorFromKeydown(
        event({ key: "F12", code: "F12", altKey: true, shiftKey: true }),
      ),
    ).toEqual({ kind: "commit", accelerator: "alt+shift+f12" });
  });

  it("commits digits, arrows and punctuation by physical key", () => {
    expect(
      acceleratorFromKeydown(event({ key: "1", code: "Digit1", ctrlKey: true })),
    ).toEqual({ kind: "commit", accelerator: "ctrl+1" });
    expect(
      acceleratorFromKeydown(
        event({ key: "ArrowUp", code: "ArrowUp", metaKey: true }),
      ),
    ).toEqual({ kind: "commit", accelerator: "super+up" });
    expect(
      acceleratorFromKeydown(event({ key: ",", code: "Comma", altKey: true })),
    ).toEqual({ kind: "commit", accelerator: "alt+comma" });
  });

  it("uses the physical key on shifted/altered layouts", () => {
    // AltGr layouts / shifted digits: event.key is "!" but the code is the
    // physical Digit1 - the accelerator must be layout-independent.
    expect(
      acceleratorFromKeydown(
        event({ key: "!", code: "Digit1", ctrlKey: true, shiftKey: true }),
      ),
    ).toEqual({ kind: "commit", accelerator: "ctrl+shift+1" });
  });

  it("rejects a bare key without modifiers", () => {
    expect(acceleratorFromKeydown(event({ key: "a", code: "KeyA" }))).toEqual({
      kind: "invalid",
      reason: "missing-modifier",
    });
    expect(acceleratorFromKeydown(event({ key: "F5", code: "F5" }))).toEqual({
      kind: "invalid",
      reason: "missing-modifier",
    });
  });

  it("treats shift-only exactly like the other modifiers", () => {
    expect(
      acceleratorFromKeydown(
        event({ key: "A", code: "KeyA", shiftKey: true }),
      ),
    ).toEqual({ kind: "commit", accelerator: "shift+a" });
  });

  it("keeps recording on a modifier-only keydown, reporting the held set", () => {
    expect(
      acceleratorFromKeydown(
        event({ key: "Control", code: "ControlLeft", ctrlKey: true }),
      ),
    ).toEqual({ kind: "pending", modifiers: ["ctrl"] });
    expect(
      acceleratorFromKeydown(
        event({
          key: "Shift",
          code: "ShiftRight",
          ctrlKey: true,
          shiftKey: true,
        }),
      ),
    ).toEqual({ kind: "pending", modifiers: ["ctrl", "shift"] });
    expect(
      acceleratorFromKeydown(
        event({ key: "Meta", code: "MetaLeft", metaKey: true }),
      ),
    ).toEqual({ kind: "pending", modifiers: ["super"] });
  });

  it("cancels on Escape, even mid-chord", () => {
    expect(
      acceleratorFromKeydown(
        event({ key: "Escape", code: "Escape", ctrlKey: true, altKey: true }),
      ),
    ).toEqual({ kind: "cancel" });
  });

  it("rejects keys the Rust parser does not know", () => {
    expect(
      acceleratorFromKeydown(
        event({ key: "CapsLock", code: "CapsLock", ctrlKey: true }),
      ),
    ).toEqual({ kind: "invalid", reason: "unsupported-key" });
    expect(
      acceleratorFromKeydown(
        event({ key: "F25", code: "F25", ctrlKey: true }),
      ),
    ).toEqual({ kind: "invalid", reason: "unsupported-key" });
    expect(
      acceleratorFromKeydown(
        event({ key: "Unidentified", code: "", ctrlKey: true }),
      ),
    ).toEqual({ kind: "invalid", reason: "unsupported-key" });
  });
});

describe("keyTokenFromCode", () => {
  it("maps letters, digits and numpad keys", () => {
    expect(keyTokenFromCode("KeyZ")).toBe("z");
    expect(keyTokenFromCode("Digit0")).toBe("0");
    expect(keyTokenFromCode("Numpad7")).toBe("numpad7");
    expect(keyTokenFromCode("NumpadAdd")).toBe("numpadadd");
  });

  it("maps the F-key range the parser accepts and rejects beyond it", () => {
    expect(keyTokenFromCode("F1")).toBe("f1");
    expect(keyTokenFromCode("F19")).toBe("f19");
    expect(keyTokenFromCode("F24")).toBe("f24");
    expect(keyTokenFromCode("F25")).toBeNull();
  });

  it("rejects lock, IME and unknown keys", () => {
    for (const code of [
      "CapsLock",
      "NumLock",
      "ScrollLock",
      "ContextMenu",
      "Lang1",
      "IntlRo",
      "MediaPlayPause",
      "",
    ]) {
      expect(keyTokenFromCode(code)).toBeNull();
    }
  });
});

describe("heldModifiers", () => {
  it("returns canonical order", () => {
    expect(
      heldModifiers(
        event({ metaKey: true, shiftKey: true, altKey: true, ctrlKey: true }),
      ),
    ).toEqual(["ctrl", "alt", "shift", "super"]);
    expect(heldModifiers(event({}))).toEqual([]);
  });
});

describe("acceleratorParts", () => {
  it("splits stored values into chips", () => {
    expect(acceleratorParts("ctrl+alt+space")).toEqual([
      "ctrl",
      "alt",
      "space",
    ]);
  });

  it("tolerates hand-typed junk from the old free-text input", () => {
    expect(acceleratorParts(" ctrl + alt + space ")).toEqual([
      "ctrl",
      "alt",
      "space",
    ]);
    expect(acceleratorParts("")).toEqual([]);
    expect(acceleratorParts("+")).toEqual([]);
  });
});

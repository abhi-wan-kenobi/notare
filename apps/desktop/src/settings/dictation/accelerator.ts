/**
 * Keyboard-event -> global-hotkey accelerator normalization for the shortcut
 * recorder in the dictation settings.
 *
 * The output format is the string syntax `tauri-plugin-global-shortcut`
 * parses on the Rust side (`plugins/shortcut/src/handler.rs`, backed by the
 * `global-hotkey` crate): lowercase, `+`-joined, modifiers first, exactly one
 * main key - e.g. `"ctrl+alt+space"`, `"ctrl+shift+f5"`, `"super+up"`.
 * Lowercase matches the existing `dictation_shortcut` convention (the schema
 * default is `"ctrl+alt+space"`).
 *
 * Pure functions over a plain-event shape so the whole matrix is unit
 * testable without DOM KeyboardEvent constructors.
 */

/** The subset of `KeyboardEvent` the recorder needs. */
export interface RecorderKeyEvent {
  key: string;
  code: string;
  ctrlKey: boolean;
  altKey: boolean;
  shiftKey: boolean;
  metaKey: boolean;
}

/** Modifier tokens in canonical output order. */
export const MODIFIER_ORDER = ["ctrl", "alt", "shift", "super"] as const;
export type ModifierToken = (typeof MODIFIER_ORDER)[number];

export type RecorderKeydownResult =
  /** A modifier-only press: show the held chips, keep recording. */
  | { kind: "pending"; modifiers: ModifierToken[] }
  /** Escape: abort recording, keep the previous value. */
  | { kind: "cancel" }
  /** A complete, normalized accelerator. */
  | { kind: "commit"; accelerator: string }
  /** Not a usable combo; keep recording and explain why. */
  | { kind: "invalid"; reason: "missing-modifier" | "unsupported-key" };

/** Modifier tokens currently held during `event`, in canonical order. */
export function heldModifiers(event: RecorderKeyEvent): ModifierToken[] {
  const held: ModifierToken[] = [];
  if (event.ctrlKey) held.push("ctrl");
  if (event.altKey) held.push("alt");
  if (event.shiftKey) held.push("shift");
  if (event.metaKey) held.push("super");
  return held;
}

/** `event.key` values that are modifiers (never a main key). */
const MODIFIER_KEYS = new Set(["Control", "Alt", "Shift", "Meta", "AltGraph"]);

/**
 * `event.code` -> accelerator key token. Every value on the right is a name
 * the `global-hotkey` parser accepts case-insensitively (its `parse_key`
 * alias table); arrows use the short `up`/`down`/`left`/`right` aliases to
 * stay readable in the settings UI.
 */
const CODE_TO_KEY: Record<string, string> = {
  Space: "space",
  Enter: "enter",
  Tab: "tab",
  Backspace: "backspace",
  Delete: "delete",
  Insert: "insert",
  Home: "home",
  End: "end",
  PageUp: "pageup",
  PageDown: "pagedown",
  ArrowUp: "up",
  ArrowDown: "down",
  ArrowLeft: "left",
  ArrowRight: "right",
  Minus: "minus",
  Equal: "equal",
  BracketLeft: "bracketleft",
  BracketRight: "bracketright",
  Backslash: "backslash",
  Semicolon: "semicolon",
  Quote: "quote",
  Comma: "comma",
  Period: "period",
  Slash: "slash",
  Backquote: "backquote",
  NumpadEnter: "numpadenter",
  NumpadAdd: "numpadadd",
  NumpadSubtract: "numpadsubtract",
  NumpadMultiply: "numpadmultiply",
  NumpadDivide: "numpaddivide",
  NumpadDecimal: "numpaddecimal",
};

/**
 * Map a physical key (`event.code`) to its accelerator token, or `null` when
 * the key cannot be part of a global hotkey (unknown hardware keys, lock
 * keys, media keys we do not offer, IME keys, ...).
 */
export function keyTokenFromCode(code: string): string | null {
  const direct = CODE_TO_KEY[code];
  if (direct) {
    return direct;
  }

  const letter = /^Key([A-Z])$/.exec(code);
  if (letter) {
    return letter[1].toLowerCase();
  }

  const digit = /^Digit([0-9])$/.exec(code);
  if (digit) {
    return digit[1];
  }

  const numpad = /^Numpad([0-9])$/.exec(code);
  if (numpad) {
    return `numpad${numpad[1]}`;
  }

  const fnKey = /^F([1-9]|1[0-9]|2[0-4])$/.exec(code);
  if (fnKey) {
    return `f${fnKey[1]}`;
  }

  return null;
}

/**
 * Classify one keydown while the recorder is armed.
 *
 * Rules: Escape cancels; a modifier keydown is a pending chord; a main key
 * commits only when >= 1 modifier is held (a global hotkey may not be a bare
 * key) and the key is one the Rust parser knows.
 */
export function acceleratorFromKeydown(
  event: RecorderKeyEvent,
): RecorderKeydownResult {
  if (event.key === "Escape") {
    return { kind: "cancel" };
  }

  const modifiers = heldModifiers(event);

  if (MODIFIER_KEYS.has(event.key)) {
    return { kind: "pending", modifiers };
  }

  const key = keyTokenFromCode(event.code);
  if (!key) {
    return { kind: "invalid", reason: "unsupported-key" };
  }

  if (modifiers.length === 0) {
    return { kind: "invalid", reason: "missing-modifier" };
  }

  return { kind: "commit", accelerator: [...modifiers, key].join("+") };
}

/**
 * Split a stored accelerator into display chips ("ctrl+alt+space" ->
 * ["ctrl", "alt", "space"]). Tolerates whatever the setting holds - the
 * hand-typed values of the old free-text input included.
 */
export function acceleratorParts(value: string): string[] {
  return value
    .split("+")
    .map((part) => part.trim())
    .filter((part) => part.length > 0);
}

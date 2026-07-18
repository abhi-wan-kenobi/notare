import { Trans, useLingui } from "@lingui/react/macro";
import { platform } from "@tauri-apps/plugin-os";
import { RotateCcwIcon } from "lucide-react";
import { useEffect, useId, useRef, useState } from "react";

import { commands as shortcutCommands } from "@hypr/plugin-shortcut";
import { Button } from "@hypr/ui/components/ui/button";
import { cn } from "@hypr/utils";

import {
  acceleratorFromKeydown,
  acceleratorParts,
  type ModifierToken,
} from "./accelerator";

/**
 * Auto-capture recorder for the dictation toggle shortcut (replaces the old
 * free-text input). Click the combo to arm it, press the shortcut you want:
 * modifiers show up as keycap chips while held, the first non-modifier key
 * completes the combo. Escape (or clicking away) cancels; a combo needs at
 * least one modifier.
 *
 * Before committing, the candidate is parse-validated through the shortcut
 * plugin (`parse_global_hotkey` - the exact parser that will register it),
 * so a bad combo surfaces inline here instead of failing silently in the
 * orb host's re-register effect.
 */
export function ShortcutRecorderRow({
  value,
  defaultValue,
  onCommit,
}: {
  value: string;
  defaultValue: string;
  onCommit: (next: string) => void;
}) {
  const { t } = useLingui();
  const titleId = useId();
  const descriptionId = useId();
  const [recording, setRecording] = useState(false);
  const [heldModifiers, setHeldModifiers] = useState<ModifierToken[]>([]);
  const [error, setError] = useState<string | null>(null);
  const buttonRef = useRef<HTMLButtonElement | null>(null);
  // Guards the async validate against a recorder torn down mid-flight.
  const sessionRef = useRef(0);

  const stopRecording = () => {
    sessionRef.current += 1;
    setRecording(false);
    setHeldModifiers([]);
  };

  useEffect(() => {
    return () => {
      sessionRef.current += 1;
    };
  }, []);

  const startRecording = () => {
    setError(null);
    setHeldModifiers([]);
    setRecording(true);
    // WebKit (macOS, and iOS/tvOS Safari) does not move DOM focus to a
    // <button> on click by default - only text inputs/links are click-
    // focusable unless "Full Keyboard Access: All Controls" is on in System
    // Settings (off by default). Without this, the button never becomes
    // `document.activeElement`, so the keydown/keyup handlers below - which
    // rely on the recorder having focus - never fire, and the recorder looks
    // like it silently ignores every combo. Chromium/Firefox already focus
    // on click, so this is a no-op there.
    buttonRef.current?.focus();
  };

  const commitCandidate = async (accelerator: string) => {
    const session = ++sessionRef.current;
    setRecording(false);
    setHeldModifiers([]);

    if (accelerator === value) {
      return;
    }

    let message: string | null = null;
    try {
      const result = await shortcutCommands.parseGlobalHotkey(accelerator);
      if (result.status === "error") {
        message = result.error;
      }
    } catch (validationError) {
      message =
        validationError instanceof Error
          ? validationError.message
          : String(validationError);
    }

    if (session !== sessionRef.current) {
      return;
    }

    if (message !== null) {
      setError(
        t`This combination cannot be used as a global shortcut. Try another one.`,
      );
      console.warn(
        `[dictation] rejected shortcut candidate "${accelerator}"`,
        message,
      );
      return;
    }

    setError(null);
    onCommit(accelerator);
  };

  const handleKeyDown = (event: React.KeyboardEvent) => {
    if (!recording) {
      // Not armed: keep normal button semantics (Enter/Space arm it via
      // onClick), everything else passes through.
      return;
    }

    event.preventDefault();
    event.stopPropagation();

    const result = acceleratorFromKeydown(event.nativeEvent);
    switch (result.kind) {
      case "cancel":
        setError(null);
        stopRecording();
        break;
      case "pending":
        setError(null);
        setHeldModifiers(result.modifiers);
        break;
      case "invalid":
        setError(
          result.reason === "missing-modifier"
            ? t`Combine at least one of Ctrl, Alt, Shift or Super with a key.`
            : t`That key cannot be part of a global shortcut.`,
        );
        setHeldModifiers([]);
        break;
      case "commit":
        setError(null);
        void commitCandidate(result.accelerator);
        break;
    }
  };

  const handleKeyUp = (event: React.KeyboardEvent) => {
    if (!recording) {
      return;
    }
    event.preventDefault();
    // Releasing a modifier mid-chord: drop it from the held preview.
    setHeldModifiers((current) =>
      current.filter((token) => {
        switch (token) {
          case "ctrl":
            return event.ctrlKey;
          case "alt":
            return event.altKey;
          case "shift":
            return event.shiftKey;
          case "super":
            return event.metaKey;
        }
      }),
    );
  };

  const chips = recording ? heldModifiers : acceleratorParts(value);
  const isDefault = value === defaultValue;
  const isMacos = platform() === "macos";

  return (
    <div className="flex items-start justify-between gap-4">
      <div className="flex-1">
        <h3 id={titleId} className="mb-1 text-sm font-medium">
          <Trans>Toggle shortcut</Trans>
        </h3>
        <p id={descriptionId} className="text-muted-foreground text-xs">
          <Trans>
            Global shortcut that starts or stops dictation. Click the combo,
            then press the keys you want - at least one of Ctrl, Alt, Shift or
            Super plus a key.
          </Trans>
        </p>
        {error ? (
          <p
            role="alert"
            data-testid="shortcut-recorder-error"
            className="text-destructive mt-1 text-xs"
          >
            {error}
          </p>
        ) : null}
      </div>
      <div className="flex items-center gap-1">
        <button
          ref={buttonRef}
          type="button"
          data-testid="shortcut-recorder"
          data-recording={recording || undefined}
          aria-labelledby={titleId}
          aria-describedby={descriptionId}
          title={recording ? t`Press the shortcut, Esc cancels` : t`Change shortcut`}
          onClick={() => {
            if (!recording) {
              startRecording();
            }
          }}
          onKeyDown={handleKeyDown}
          onKeyUp={handleKeyUp}
          onBlur={() => {
            if (recording) {
              stopRecording();
            }
          }}
          className={cn([
            "flex h-8 min-w-40 cursor-pointer items-center justify-center gap-1 rounded-lg border px-2",
            "transition-colors duration-(--motion-duration-state)",
            recording
              ? "border-primary/60 ring-primary/40 bg-accent/40 ring-1"
              : "border-border hover:bg-accent/20",
          ])}
        >
          {chips.length > 0 ? (
            <>
              {chips.map((part, index) => (
                <KeycapChip
                  key={`${part}-${index}`}
                  label={part}
                  isMacos={isMacos}
                />
              ))}
              {recording ? (
                <span className="text-muted-foreground text-xs">…</span>
              ) : null}
            </>
          ) : (
            <span className="text-muted-foreground text-xs">
              {recording ? (
                <Trans>Press shortcut…</Trans>
              ) : (
                <Trans>Not set</Trans>
              )}
            </span>
          )}
        </button>
        {!isDefault ? (
          <Button
            variant="ghost"
            size="icon"
            aria-label={t`Reset to the default shortcut`}
            title={t`Reset to the default shortcut`}
            onClick={() => {
              setError(null);
              stopRecording();
              onCommit(defaultValue);
            }}
          >
            <RotateCcwIcon className="size-3.5" />
          </Button>
        ) : null}
      </div>
    </div>
  );
}

/** Presentation of one accelerator token as a keycap. */
function KeycapChip({
  label,
  isMacos,
}: {
  label: string;
  isMacos: boolean;
}) {
  return (
    <kbd
      className={cn([
        "border-border bg-muted text-foreground rounded-md border px-1.5 py-0.5",
        "font-mono text-[11px] leading-none shadow-[inset_0_-1px_0_hsl(var(--border))]",
      ])}
    >
      {formatKeyToken(label, isMacos)}
    </kbd>
  );
}

/**
 * macOS keyboard-symbol convention for the four modifiers (System Settings >
 * Keyboard Shortcuts renders them the same way) - the physical keys these
 * tokens map to on a Mac keyboard are Control, Option, Shift and Command
 * (`heldModifiers` in `./accelerator.ts` maps `event.metaKey` - Cmd on macOS
 * - to "super"). Everything else keeps the spelled-out Windows/Linux label.
 */
const MAC_MODIFIER_GLYPHS: Partial<Record<string, string>> = {
  ctrl: "⌃",
  alt: "⌥",
  shift: "⇧",
  super: "⌘",
};

/**
 * "ctrl" -> "Ctrl" (Windows/Linux) or "⌃" (macOS); "pageup" -> "PageUp";
 * "f5" -> "F5"; "up" -> "↑".
 */
function formatKeyToken(token: string, isMacos: boolean): string {
  if (isMacos) {
    const macGlyph = MAC_MODIFIER_GLYPHS[token];
    if (macGlyph) {
      return macGlyph;
    }
  }

  const special: Record<string, string> = {
    ctrl: "Ctrl",
    alt: "Alt",
    shift: "Shift",
    super: "Super",
    space: "Space",
    enter: "Enter",
    tab: "Tab",
    backspace: "Backspace",
    delete: "Del",
    insert: "Ins",
    home: "Home",
    end: "End",
    pageup: "PgUp",
    pagedown: "PgDn",
    up: "↑",
    down: "↓",
    left: "←",
    right: "→",
    backquote: "`",
    minus: "-",
    equal: "=",
    bracketleft: "[",
    bracketright: "]",
    backslash: "\\",
    semicolon: ";",
    quote: "'",
    comma: ",",
    period: ".",
    slash: "/",
  };
  const known = special[token];
  if (known) {
    return known;
  }
  if (/^f([1-9]|1[0-9]|2[0-4])$/.test(token)) {
    return token.toUpperCase();
  }
  if (token.startsWith("numpad")) {
    return `Num ${token.slice("numpad".length)}`;
  }
  return token.length === 1 ? token.toUpperCase() : token;
}

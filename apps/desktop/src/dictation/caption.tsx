import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { useEffect, useRef, useState } from "react";

import {
  type DictationStateEvent,
  events as dictationEvents,
} from "@hypr/plugin-dictation";
import { cn } from "@hypr/utils";

import { useConfigValues } from "~/shared/config";

/**
 * Content of the tiny live-caption webview window that floats just above the
 * dictation orb (Windows/Linux). The Rust side of the dictation plugin
 * creates it next to the orb window (`plugins/dictation/src/orb.rs`), keeps
 * it glued above the orb on move/resize and marks it click-through via
 * `set_ignore_cursor_events(true)` - it can never intercept a click, which
 * is why it is a second window instead of an enlarged orb window.
 *
 * While dictating, the last ~10 recognized words stream in through
 * `DictationTranscriptEvent` (the same final-segment event both output modes
 * emit). The caption fades out after ~2s without new words and shortly after
 * the session ends; this webview shows/hides its own OS window around that
 * so the (possibly opaque, `?solid=1` fallback) surface is only ever on
 * screen while text is.
 *
 * Gated by the `dictation_caption` setting ("Show live caption over orb").
 */

/** How many trailing words the caption shows. */
const CAPTION_WORD_COUNT = 10;
/** Fade out after this long without a new transcript segment. */
const CAPTION_IDLE_FADE_MS = 2000;
/** Faster fade once the session has ended. */
const CAPTION_SESSION_END_FADE_MS = 1200;
/** Opacity transition length; the OS window hides after it completes. */
const CAPTION_FADE_TRANSITION_MS = 220;

export function DictationCaptionWindow({ solid = false }: { solid?: boolean }) {
  const { dictation_caption } = useConfigValues([
    "dictation_caption",
  ] as const);
  const enabled = dictation_caption;

  const [words, setWords] = useState<string[]>([]);
  const [visible, setVisible] = useState(false);
  const phaseRef = useRef<DictationStateEvent["phase"]>("idle");
  const fadeTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    // Same chrome treatment as the orb window: dark token set, transparent
    // page unless the OS forced the solid fallback.
    document.documentElement.classList.add("dark");

    if (solid) {
      return;
    }

    document.documentElement.style.background = "transparent";
    document.body.style.background = "transparent";
  }, [solid]);

  useEffect(() => {
    if (!enabled) {
      setWords([]);
      setVisible(false);
      return;
    }

    let cancelled = false;
    const unlisteners: (() => void)[] = [];
    const collect = (promise: Promise<() => void>) => {
      void promise.then((unlisten) => {
        if (cancelled) {
          unlisten();
          return;
        }
        unlisteners.push(unlisten);
      });
    };

    const scheduleFade = (delay: number) => {
      if (fadeTimerRef.current !== null) {
        clearTimeout(fadeTimerRef.current);
      }
      fadeTimerRef.current = setTimeout(() => {
        fadeTimerRef.current = null;
        setVisible(false);
      }, delay);
    };

    collect(
      dictationEvents.dictationTranscriptEvent.listen(({ payload }) => {
        const incoming = payload.text.split(/\s+/).filter(Boolean);
        if (incoming.length === 0) {
          return;
        }
        setWords((current) =>
          [...current, ...incoming].slice(-CAPTION_WORD_COUNT),
        );
        setVisible(true);
        scheduleFade(CAPTION_IDLE_FADE_MS);
      }),
    );

    collect(
      dictationEvents.dictationStateEvent.listen(({ payload }) => {
        const previous = phaseRef.current;
        phaseRef.current = payload.phase;

        if (
          payload.phase === "listening" &&
          (previous === "idle" || previous === "error")
        ) {
          // New session: drop the previous session's tail.
          setWords([]);
          setVisible(false);
        }

        if (payload.phase === "idle" || payload.phase === "error") {
          scheduleFade(CAPTION_SESSION_END_FADE_MS);
        }
      }),
    );

    return () => {
      cancelled = true;
      for (const unlisten of unlisteners) {
        unlisten();
      }
      if (fadeTimerRef.current !== null) {
        clearTimeout(fadeTimerRef.current);
        fadeTimerRef.current = null;
      }
    };
  }, [enabled]);

  // Mirror `visible` onto the OS window so the caption surface (opaque in
  // the solid fallback) is only on screen while text is showing. The hide
  // waits out the opacity transition.
  useEffect(() => {
    const window = safeCaptionWindow();
    if (!window) {
      return;
    }

    if (visible) {
      void window.show().catch(() => {});
      return;
    }

    const timeout = setTimeout(() => {
      void window.hide().catch(() => {});
    }, CAPTION_FADE_TRANSITION_MS);
    return () => {
      clearTimeout(timeout);
    };
  }, [visible]);

  const text = words.join(" ");

  return (
    <div
      data-testid={
        solid ? "dictation-caption-solid" : "dictation-caption-glass"
      }
      // Belt and braces: the Rust side already makes the window
      // click-through; pointer-events none keeps the DOM inert too.
      className="pointer-events-none flex h-screen w-screen items-end justify-center overflow-hidden pb-1"
    >
      <div
        data-testid="dictation-caption-bubble"
        aria-hidden={!visible || !text}
        className={cn([
          "max-w-full rounded-lg px-3 py-1.5",
          "bg-background/90 border-border border",
          "transition-opacity duration-200 ease-out motion-reduce:transition-none",
          visible && text ? "opacity-100" : "opacity-0",
        ])}
      >
        <p
          data-testid="dictation-caption-text"
          className="text-foreground line-clamp-2 text-center text-xs leading-snug"
        >
          {text}
        </p>
      </div>
    </div>
  );
}

function safeCaptionWindow() {
  try {
    return getCurrentWebviewWindow();
  } catch {
    // Not running inside the caption webview (tests/storybook).
    return null;
  }
}

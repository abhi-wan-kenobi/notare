import { useLingui } from "@lingui/react/macro";
import { platform } from "@tauri-apps/plugin-os";
import { generateText } from "ai";
import { useCallback, useEffect, useMemo, useRef } from "react";

import {
  commands as dictationCommands,
  type DictationFinishedEvent,
  type DictationOutputMode,
  type DictationPhase,
  events as dictationEvents,
} from "@hypr/plugin-dictation";
import {
  commands as shortcutCommands,
  events as shortcutEvents,
} from "@hypr/plugin-shortcut";
import { sonnerToast } from "@hypr/ui/components/ui/toast";

import { type DictionaryEntry, parseDictionaryEntries } from "./dictionary";
import { finalizeDictation, normalizeCleanupMode } from "./finalize";
import { isLikelyEngineBusyError } from "./errors";
import { addDictationHistoryEntry, listDictationHistory } from "./history";
import { isLegacyOutputMode, normalizeOutputMode } from "./output-mode";

import { useScopedLanguageModel } from "~/ai/hooks";
import { deterministicGenerationSettings } from "~/ai/model-settings";
import { useSetSettingValues, useStoredSettingValue } from "~/settings/queries";
import { useConfigValues } from "~/shared/config";
import { listenerStore } from "~/store/zustand/listener/instance";
import { useSTTConnection } from "~/stt/useSTTConnection";

/**
 * Keyed-hotkey registration ids for the shortcut plugin. Two global hotkeys run
 * concurrently off the same `globalHotkeyTriggered` event, distinguished by the
 * `id` the plugin echoes back: the dictation toggle and the paste-last-dictation
 * shortcut.
 */
const HOTKEY_ID_TOGGLE = "dictation_toggle";
const HOTKEY_ID_PASTE_LAST = "dictation_paste_last";

/**
 * Main-window controller for the persistent dictation orb, active on every
 * platform since #31 - macOS reaches parity through this same webview orb
 * instead of its unfinished native panel.
 *
 * Responsibilities:
 * - show/hide the orb window when the `dictation_enabled` setting changes,
 *   keeping it hidden while a meeting recording is live or after the user
 *   right-clicked it away (both until the next dictation session starts);
 * - register the configured global toggle hotkey (`dictation_shortcut`);
 * - toggle the Rust dictation session on hotkey press or orb click, passing
 *   the live local STT server URL + model from `useSTTConnection`;
 * - finish each session: the Rust side emits `DictationFinishedEvent` with
 *   the raw transcript, and this host applies the configured cleanup
 *   (basic/LLM), delivers batch-mode text (paste at cursor or copy-only,
 *   per `dictation_paste_at_cursor`) and records the history entry.
 *
 * The session itself (mic capture, websocket to the local whisper server,
 * live text injection) runs entirely in the dictation plugin's Rust side.
 */
export function DictationOrbHost() {
  const { t } = useLingui();
  const isMacos = platform() === "macos";
  const {
    dictation_enabled,
    dictation_shortcut,
    dictation_paste_last_shortcut,
    dictation_output_mode,
    dictation_paste_at_cursor,
    dictation_cleanup,
    dictation_translation_enabled,
    dictation_translation_target,
  } = useConfigValues([
    "dictation_enabled",
    "dictation_shortcut",
    "dictation_paste_last_shortcut",
    "dictation_output_mode",
    "dictation_paste_at_cursor",
    "dictation_cleanup",
    "dictation_translation_enabled",
    "dictation_translation_target",
  ] as const);
  const setSettingValues = useSetSettingValues();
  const enabled = dictation_enabled;

  const outputMode: DictationOutputMode = normalizeOutputMode(
    dictation_output_mode,
  );
  const outputModeRef = useRef(outputMode);
  outputModeRef.current = outputMode;

  const finalizeSettingsRef = useRef({
    cleanup: normalizeCleanupMode(dictation_cleanup),
    pasteAtCursor: dictation_paste_at_cursor,
    translation: {
      enabled: dictation_translation_enabled,
      target: dictation_translation_target,
    },
  });
  finalizeSettingsRef.current = {
    cleanup: normalizeCleanupMode(dictation_cleanup),
    pasteAtCursor: dictation_paste_at_cursor,
    translation: {
      enabled: dictation_translation_enabled,
      target: dictation_translation_target,
    },
  };

  // Custom-dictionary entries. Read the RAW stored setting string (not
  // `useConfigValue`, which strips the mapping objects down to plain strings)
  // so the deterministic wrong -> right replacements survive to the finalize
  // pass. Kept in a ref so the finished-event handler sees the latest.
  const { value: dictionaryRaw } = useStoredSettingValue(
    "personalization_dictionary_terms",
  );
  const dictionaryEntries = useMemo<DictionaryEntry[]>(
    () =>
      parseDictionaryEntries(
        typeof dictionaryRaw === "string" ? dictionaryRaw : "[]",
      ),
    [dictionaryRaw],
  );
  const dictionaryRef = useRef(dictionaryEntries);
  dictionaryRef.current = dictionaryEntries;

  // LLM cleanup/translation uses the "cleanup" scope's model (its per-scope
  // override, or the global selection); null = not configured.
  const model = useScopedLanguageModel("cleanup");
  const modelRef = useRef(model);
  modelRef.current = model;

  // One-time migration of the pre-rework setting value: "batch-paste" was
  // batch mode with the paste baked in, so it becomes "batch" + the
  // paste-at-cursor toggle on.
  useEffect(() => {
    if (!isMacos && isLegacyOutputMode(dictation_output_mode)) {
      setSettingValues({
        dictation_output_mode: "batch",
        dictation_paste_at_cursor: true,
      });
    }
  }, [isMacos, dictation_output_mode, setSettingValues]);

  const { conn, isLocalModel } = useSTTConnection();
  // Dictation streams to the internal whisper server, so only local models
  // are supported for now.
  const localConn = isLocalModel ? conn : null;
  const connRef = useRef(localConn);
  connRef.current = localConn;

  const phaseRef = useRef<DictationPhase>("idle");

  // Wall-clock starts of sessions whose finished event hasn't arrived yet,
  // oldest first. A FIFO (not a single slot) so a rapid stop+restart pairs
  // each late-arriving finished event with its own session's start instead
  // of the newest one's.
  const sessionStartsRef = useRef<number[]>([]);

  const toggle = useCallback(() => {
    if (phaseRef.current === "listening" || phaseRef.current === "processing") {
      void dictationCommands.stopDictation();
      return;
    }

    const conn = connRef.current;
    if (!conn) {
      // No local live model is configured/downloaded — surface it instead of
      // silently swallowing the orb click (the pre-split no-op regression).
      console.warn(
        "[dictation] no local STT model ready; select and download a local " +
          "transcription model before dictating",
      );
      sonnerToast.info(
        t`Dictation needs a downloaded local model — choose one in Settings.`,
      );
      return;
    }

    const startedAt = Date.now();
    // A session that died without ever emitting a finished event would leave
    // its timestamp queued forever and mispair every later finish - drop
    // anything implausibly old before enqueueing (no dictation runs 6h).
    sessionStartsRef.current = sessionStartsRef.current.filter(
      (start) => startedAt - start < 6 * 60 * 60 * 1000,
    );
    sessionStartsRef.current.push(startedAt);
    // A start that fails produces no finished event - drop its timestamp so
    // it can't get paired with a later session's finish.
    const dropStart = () => {
      const index = sessionStartsRef.current.indexOf(startedAt);
      if (index >= 0) {
        sessionStartsRef.current.splice(index, 1);
      }
    };
    void dictationCommands
      .startDictation(conn.baseUrl, conn.model, outputModeRef.current)
      .then((result) => {
        if (result.status === "error") {
          dropStart();
          // Surface it instead of a silent no-op orb click/hotkey press. The
          // most common real cause is engine contention (a batch re-transcription
          // is already using the internal whisper server); otherwise it's e.g.
          // the local server not up yet or the macOS Soniqo bridge failing.
          // Never dump the raw backend error at the user — log it, show guidance.
          console.error(
            "[dictation] failed to start the dictation session",
            result.error,
          );
          sonnerToast.error(
            isLikelyEngineBusyError(result.error)
              ? t`Couldn't start dictation — the transcription engine is busy. If a recording is still transcribing, try again once it finishes.`
              : t`Couldn't start dictation. Check that a local model is selected, then try again.`,
          );
        }
      })
      .catch((error) => {
        dropStart();
        console.error(
          "[dictation] failed to start the dictation session",
          error,
        );
        sonnerToast.error(
          t`Couldn't start dictation. Check that a local model is selected, then try again.`,
        );
      });
  }, [t]);

  // Guards the paste-last hotkey against re-entrancy: a fetch + deliver is
  // async, and holding/mashing the hotkey must not fire a second paste while
  // one is still in flight (which would double-paste, or race the clipboard
  // save/restore in `deliverText`). One paste at a time.
  const pasteLastInFlightRef = useRef(false);

  // Paste-last-dictation hotkey handler: fetch the newest *delivered* history
  // entry with non-empty text and paste it at the cursor (with clipboard
  // restore). Empty history or a failed delivery surfaces a toast - never a
  // silent no-op.
  const pasteLast = useCallback(async () => {
    if (pasteLastInFlightRef.current) {
      return;
    }
    if (phaseRef.current === "listening" || phaseRef.current === "processing") {
      // Pasting mid-session would interleave the previous transcript with the
      // live one in whatever app has focus.
      sonnerToast.info(
        t`Finish the current dictation before pasting the last one.`,
      );
      return;
    }
    pasteLastInFlightRef.current = true;
    try {
      // Query a few rows, not just one: the newest row can be a `discarded`
      // (recovery-only) entry, which must be skipped in favour of the newest
      // actually-delivered one.
      const { entries } = await listDictationHistory({ limit: 5 });
      const entry = entries.find(
        (candidate) =>
          candidate.status === "delivered" && candidate.text.trim().length > 0,
      );
      if (!entry) {
        sonnerToast.info(t`No dictation to paste yet.`);
        return;
      }
      const result = await dictationCommands.deliverText(entry.text, true);
      if (result.status === "error") {
        console.error(
          "[dictation] failed to paste the last dictation",
          result.error,
        );
        sonnerToast.error(t`Couldn't paste the last dictation. Try again.`);
      }
    } catch (error) {
      console.error("[dictation] failed to paste the last dictation", error);
      sonnerToast.error(t`Couldn't paste the last dictation. Try again.`);
    } finally {
      pasteLastInFlightRef.current = false;
    }
  }, [t]);

  const handleFinished = useCallback(
    async (event: DictationFinishedEvent) => {
      const settings = finalizeSettingsRef.current;
      const model = modelRef.current;
      const startedAt = sessionStartsRef.current.shift();
      const durationMs = startedAt != null ? Date.now() - startedAt : null;

      try {
        await finalizeDictation(
          {
            rawText: event.rawText,
            mode: event.mode,
            failed: event.failed,
            cleanup: settings.cleanup,
            dictionary: dictionaryRef.current,
            translation: settings.translation,
            pasteAtCursor: settings.pasteAtCursor,
            // The STT model name comes from the live connection, not the
            // finished event; `connRef` still holds the session's model.
            model: connRef.current?.model ?? null,
            durationMs,
          },
          {
            cleanBasic: async (text) =>
              unwrap(await dictationCommands.cleanText(text)),
            cleanLlm: model
              ? async (text, systemPrompt, signal) => {
                  const result = await generateText({
                    model,
                    system: systemPrompt,
                    prompt: text,
                    // Forward the finalize pipeline's timeout abort so a
                    // wedged provider call actually stops instead of running
                    // on after the fallback paste.
                    abortSignal: signal,
                    ...deterministicGenerationSettings(model),
                  });
                  return result.text;
                }
              : null,
            deliver: async (text, pasteAtCursor) => {
              unwrap(await dictationCommands.deliverText(text, pasteAtCursor));
            },
            saveHistory: addDictationHistoryEntry,
            // Keep the orb (and phaseRef, via the state listener) in
            // "processing" while cleanup + paste run: the Rust session
            // already emitted idle before the finished event was handled.
            signalPhase: (phase) => {
              dictationEvents.dictationStateEvent
                .emit({ phase, amplitude: 0, mode: event.mode })
                .catch((error) => {
                  console.warn(
                    "[dictation] failed to broadcast the finalize phase",
                    error,
                  );
                });
            },
            onLlmFallback: (error) => {
              if (error != null) {
                console.warn("[dictation] LLM cleanup failed", error);
              }
              sonnerToast.info(
                error == null
                  ? t`No AI model is configured for dictation cleanup - used basic cleanup instead.`
                  : t`AI cleanup failed - used basic cleanup instead.`,
              );
            },
          },
        );
      } catch (error) {
        console.error("[dictation] failed to finalize the dictation", error);
      }
    },
    [t],
  );

  // Orb window lifecycle + visibility.
  //
  // The orb is visible while dictation is enabled, EXCEPT:
  // - while a meeting recording is live (the floating meeting bar already
  //   marks "recording", and a second always-on-top indicator reads as
  //   "something else is listening too"), or
  // - after the user right-clicked the orb to dismiss it.
  // Both suppressions lift the moment a dictation session actually starts
  // (phase -> listening): starting dictation is an explicit "I'm using this"
  // signal, and a live mic must never run without its indicator - including
  // mid-meeting. The right-click dismissal is deliberately closure-scoped:
  // toggling `dictation_enabled` off and back on also forgets it.
  useEffect(() => {
    if (!enabled) {
      return;
    }

    let cancelled = false;
    let meetingActive = listenerStore.getState().live.status === "active";
    let userHidden = false;
    let dictating = false;
    // Last visibility handed to the queue; `null` forces the initial sync
    // and marks "reality unknown" after a failed apply so the next
    // transition retries instead of short-circuiting.
    let shown: boolean | null = null;
    // show/hide are async IPC, not synchronous side-effects: two rapid
    // transitions would otherwise race their command round-trips and the
    // window could end on the older intent. Serializing through this queue
    // keeps applies in order; the supersede check below coalesces a burst
    // of flips into the newest intent only.
    let queue: Promise<unknown> = Promise.resolve();

    const sync = () => {
      if (cancelled) {
        return;
      }
      const visible = !userHidden && (dictating || !meetingActive);
      if (shown === visible) {
        return;
      }
      shown = visible;
      queue = queue.then(() => {
        if (cancelled || shown !== visible) {
          // Torn down, or superseded by a newer intent already queued.
          return;
        }
        const apply = visible
          ? dictationCommands.showOrb
          : dictationCommands.hideOrb;
        return apply().then((result) => {
          if (result.status === "error") {
            if (shown === visible) {
              shown = null;
            }
            console.error(
              `[dictation] failed to ${visible ? "show" : "hide"} the orb window`,
              result.error,
            );
          }
        });
      });
    };

    sync();

    // The effect can mount with a session already live (e.g. the previous
    // effect run's async stop failed) - dictating must not default to a
    // value that hides the indicator of a running mic. State events are the
    // source of truth from here on; this only seeds the initial value.
    void dictationCommands
      .isDictating()
      .then((result) => {
        if (cancelled || result.status !== "ok" || !result.data) {
          return;
        }
        if (!dictating) {
          dictating = true;
          sync();
        }
      })
      .catch((error) => {
        console.error("[dictation] failed to query the session state", error);
      });

    const unsubscribeListener = listenerStore.subscribe((state) => {
      const active = state.live.status === "active";
      if (active === meetingActive) {
        return;
      }
      meetingActive = active;
      sync();
    });

    const unlisteners: (() => void)[] = [];
    const collect = (promise: Promise<() => void>) => {
      promise
        .then((unlisten) => {
          if (cancelled) {
            unlisten();
            return;
          }
          unlisteners.push(unlisten);
        })
        .catch((error) => {
          console.error(
            "[dictation] failed to subscribe to a dictation event",
            error,
          );
        });
    };

    collect(
      dictationEvents.dictationStateEvent.listen((event) => {
        const phase = event.payload.phase;
        const active = phase === "listening" || phase === "processing";
        if (active && !dictating) {
          // A session actually started: any right-click dismissal is over.
          userHidden = false;
        }
        dictating = active;
        sync();
      }),
    );
    collect(
      dictationEvents.dictationOrbHideRequested.listen(() => {
        if (dictating) {
          // A stale right-click racing a session start (the orb window's own
          // guard reads an async copy of the phase): a live mic must never
          // lose its indicator, so the host re-checks with fresher state.
          return;
        }
        userHidden = true;
        sync();
      }),
    );

    return () => {
      cancelled = true;
      unsubscribeListener();
      for (const unlisten of unlisteners) {
        unlisten();
      }
      void dictationCommands.stopDictation();
      // Ride the queue so the final hide lands after any still-in-flight
      // apply - a show resolving late must not leave an orphaned orb.
      void queue.then(() => dictationCommands.hideOrb());
    };
  }, [enabled]);

  // Global toggle hotkey.
  useEffect(() => {
    if (!enabled || !dictation_shortcut) {
      return;
    }

    void shortcutCommands
      .registerGlobalHotkey(HOTKEY_ID_TOGGLE, dictation_shortcut)
      .then((result) => {
        if (result.status === "error") {
          console.error(
            `[dictation] failed to register hotkey "${dictation_shortcut}"`,
            result.error,
          );
        }
      });

    return () => {
      void shortcutCommands.unregisterGlobalHotkey(HOTKEY_ID_TOGGLE);
    };
  }, [enabled, dictation_shortcut]);

  // Global paste-last-dictation hotkey (independent second registration).
  // Gated on `dictation_enabled` like the toggle, and skipped when it would
  // collide with the toggle's own combo (the toggle wins - registering the same
  // accelerator twice would fail on the second binding).
  useEffect(() => {
    if (!enabled || !dictation_paste_last_shortcut) {
      return;
    }
    // Case-insensitive: accelerator strings are case-insensitive to the OS,
    // and a hand-edited setting ("Ctrl+Alt+V" vs "ctrl+alt+v") must not slip
    // past the collision check.
    if (
      dictation_paste_last_shortcut.toLowerCase() ===
      dictation_shortcut?.toLowerCase()
    ) {
      console.warn(
        "[dictation] paste-last hotkey matches the toggle hotkey; skipping " +
          "its registration so the toggle keeps working",
      );
      return;
    }

    void shortcutCommands
      .registerGlobalHotkey(HOTKEY_ID_PASTE_LAST, dictation_paste_last_shortcut)
      .then((result) => {
        if (result.status === "error") {
          console.error(
            `[dictation] failed to register paste-last hotkey "${dictation_paste_last_shortcut}"`,
            result.error,
          );
        }
      });

    return () => {
      void shortcutCommands.unregisterGlobalHotkey(HOTKEY_ID_PASTE_LAST);
    };
  }, [enabled, dictation_paste_last_shortcut, dictation_shortcut]);

  // Session-phase tracking + toggle triggers (hotkey, orb click) + finalize.
  useEffect(() => {
    if (!enabled) {
      phaseRef.current = "idle";
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

    collect(
      dictationEvents.dictationStateEvent.listen((event) => {
        phaseRef.current = event.payload.phase;
      }),
    );
    collect(
      dictationEvents.dictationFinishedEvent.listen((event) => {
        void handleFinished(event.payload);
      }),
    );
    collect(
      shortcutEvents.globalHotkeyTriggered.listen((event) => {
        if (event.payload.id === HOTKEY_ID_PASTE_LAST) {
          void pasteLast();
        } else {
          toggle();
        }
      }),
    );
    collect(dictationEvents.dictationOrbClicked.listen(() => toggle()));

    return () => {
      cancelled = true;
      for (const unlisten of unlisteners) {
        unlisten();
      }
    };
  }, [enabled, toggle, handleFinished, pasteLast]);

  return null;
}

/** Unwrap a specta `Result`-style command response, throwing the error. */
function unwrap<T>(
  result: { status: "ok"; data: T } | { status: "error"; error: string },
): T {
  if (result.status === "error") {
    throw new Error(result.error);
  }
  return result.data;
}

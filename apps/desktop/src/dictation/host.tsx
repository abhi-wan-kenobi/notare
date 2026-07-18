import { useLingui } from "@lingui/react/macro";
import { platform } from "@tauri-apps/plugin-os";
import { generateText } from "ai";
import { useCallback, useEffect, useRef } from "react";

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

import {
  finalizeDictation,
  LLM_CLEANUP_SYSTEM_PROMPT,
  normalizeCleanupMode,
} from "./finalize";
import { addDictationHistoryEntry } from "./history";
import { isLegacyOutputMode, normalizeOutputMode } from "./output-mode";

import { useLanguageModel } from "~/ai/hooks";
import { deterministicGenerationSettings } from "~/ai/model-settings";
import { useSetSettingValues } from "~/settings/queries";
import { useConfigValues } from "~/shared/config";
import { useSTTConnection } from "~/stt/useSTTConnection";

/**
 * Main-window controller for the persistent dictation orb, active on every
 * platform since #31 - macOS reaches parity through this same webview orb
 * instead of its unfinished native panel.
 *
 * Responsibilities:
 * - show/hide the orb window when the `dictation_enabled` setting changes;
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
    dictation_output_mode,
    dictation_paste_at_cursor,
    dictation_cleanup,
  } = useConfigValues([
    "dictation_enabled",
    "dictation_shortcut",
    "dictation_output_mode",
    "dictation_paste_at_cursor",
    "dictation_cleanup",
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
  });
  finalizeSettingsRef.current = {
    cleanup: normalizeCleanupMode(dictation_cleanup),
    pasteAtCursor: dictation_paste_at_cursor,
  };

  // LLM cleanup uses the app's configured provider; null = not configured.
  const model = useLanguageModel();
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

    void dictationCommands.startDictation(
      conn.baseUrl,
      conn.model,
      outputModeRef.current,
    );
  }, [t]);

  const handleFinished = useCallback(
    async (event: DictationFinishedEvent) => {
      const settings = finalizeSettingsRef.current;
      const model = modelRef.current;

      try {
        await finalizeDictation(
          {
            rawText: event.rawText,
            mode: event.mode,
            failed: event.failed,
            cleanup: settings.cleanup,
            pasteAtCursor: settings.pasteAtCursor,
          },
          {
            cleanBasic: async (text) =>
              unwrap(await dictationCommands.cleanText(text)),
            cleanLlm: model
              ? async (text) => {
                  const result = await generateText({
                    model,
                    system: LLM_CLEANUP_SYSTEM_PROMPT,
                    prompt: text,
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

  // Orb window lifecycle.
  useEffect(() => {
    if (!enabled) {
      return;
    }

    void dictationCommands.showOrb().then((result) => {
      if (result.status === "error") {
        console.error(
          "[dictation] failed to show the orb window",
          result.error,
        );
      }
    });

    return () => {
      void dictationCommands.stopDictation();
      void dictationCommands.hideOrb();
    };
  }, [enabled]);

  // Global toggle hotkey.
  useEffect(() => {
    if (!enabled || !dictation_shortcut) {
      return;
    }

    void shortcutCommands
      .registerGlobalHotkey(dictation_shortcut)
      .then((result) => {
        if (result.status === "error") {
          console.error(
            `[dictation] failed to register hotkey "${dictation_shortcut}"`,
            result.error,
          );
        }
      });

    return () => {
      void shortcutCommands.unregisterGlobalHotkey();
    };
  }, [enabled, dictation_shortcut]);

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
    collect(shortcutEvents.globalHotkeyTriggered.listen(() => toggle()));
    collect(dictationEvents.dictationOrbClicked.listen(() => toggle()));

    return () => {
      cancelled = true;
      for (const unlisten of unlisteners) {
        unlisten();
      }
    };
  }, [enabled, toggle, handleFinished]);

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

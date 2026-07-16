import { platform } from "@tauri-apps/plugin-os";
import { useCallback, useEffect, useRef } from "react";

import {
  commands as dictationCommands,
  type DictationOutputMode,
  type DictationPhase,
  events as dictationEvents,
} from "@hypr/plugin-dictation";
import {
  commands as shortcutCommands,
  events as shortcutEvents,
} from "@hypr/plugin-shortcut";

import { useConfigValues } from "~/shared/config";
import { useSTTConnection } from "~/stt/useSTTConnection";

/**
 * Main-window controller for the persistent dictation orb (Windows/Linux;
 * macOS keeps its native dictation path and this host is inert there).
 *
 * Responsibilities:
 * - show/hide the orb window when the `dictation_enabled` setting changes;
 * - register the configured global toggle hotkey (`dictation_shortcut`);
 * - toggle the Rust dictation session on hotkey press or orb click, passing
 *   the live local STT server URL + model from `useSTTConnection`.
 *
 * The session itself (mic capture, websocket to the local whisper server,
 * text injection) runs entirely in the dictation plugin's Rust side; this
 * component only orchestrates.
 */
export function DictationOrbHost() {
  const isMacos = platform() === "macos";
  const { dictation_enabled, dictation_shortcut, dictation_output_mode } =
    useConfigValues([
      "dictation_enabled",
      "dictation_shortcut",
      "dictation_output_mode",
    ] as const);
  const enabled = !isMacos && dictation_enabled;

  // Sanitize the stored string; unknown values fall back to live typing.
  const outputMode: DictationOutputMode =
    dictation_output_mode === "batch-paste" ? "batch-paste" : "type";
  const outputModeRef = useRef(outputMode);
  outputModeRef.current = outputMode;

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
      console.warn(
        "[dictation] no local STT model ready; select and download a local " +
          "transcription model before dictating",
      );
      return;
    }

    void dictationCommands.startDictation(
      conn.baseUrl,
      conn.model,
      outputModeRef.current,
    );
  }, []);

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

  // Session-phase tracking + toggle triggers (hotkey, orb click).
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
    collect(shortcutEvents.globalHotkeyTriggered.listen(() => toggle()));
    collect(dictationEvents.dictationOrbClicked.listen(() => toggle()));

    return () => {
      cancelled = true;
      for (const unlisten of unlisteners) {
        unlisten();
      }
    };
  }, [enabled, toggle]);

  return null;
}

export const SETTING_DEFINITIONS = {
  autostart: {
    type: "boolean",
    path: ["general", "autostart"],
    default: false as boolean,
  },
  auto_stop_meetings: {
    type: "boolean",
    path: ["general", "auto_stop_meetings"],
    default: true as boolean,
  },
  auto_start_scheduled_meetings: {
    type: "boolean",
    path: ["general", "auto_start_scheduled_meetings"],
    default: true as boolean,
  },
  floating_bar_enabled: {
    type: "boolean",
    path: ["general", "floating_bar_enabled"],
    default: true as boolean,
  },
  // Look of the meeting floating bar (cross-platform): "notare" (the default
  // glass orb bar, docs/DESIGN-DIRECTION.md §3b) or "classic" (the compact
  // parchment/olive pill ported from the earlier native macOS NSPanel,
  // FloatingBarView.swift). Mirrors `dictation_orb_variant`'s conventions.
  meeting_bar_theme: {
    type: "string",
    path: ["general", "meeting_bar_theme"],
    default: "notare" as string,
  },
  dictation_enabled: {
    type: "boolean",
    path: ["general", "dictation_enabled"],
    default: false as boolean,
  },
  dictation_shortcut: {
    type: "string",
    path: ["general", "dictation_shortcut"],
    default: "ctrl+alt+space" as string,
  },
  // How the dictation toggle hotkey behaves: "toggle" (press once to start,
  // again to stop - the default) or "push_to_talk" (hold to record, release to
  // stop). PTT starts on key-down and stops on key-up; toggle acts on key-down
  // only. Both read from the same `dictation_shortcut` binding.
  dictation_activation_mode: {
    type: "string",
    path: ["general", "dictation_activation_mode"],
    default: "toggle" as string,
  },
  // Auto-pause any playing media (music/video) while a dictation session runs,
  // resuming only what WE paused when it ends. Best-effort per platform
  // (MPRIS on Linux, GSMTC on Windows, Music/Spotify via osascript on macOS).
  dictation_pause_media: {
    type: "boolean",
    path: ["general", "dictation_pause_media"],
    default: false as boolean,
  },
  // Lane B2 "warm-mic": keep a microphone capture stream open while dictation
  // is idle so a hotkey press skips the OS mic-open latency. OFF by default and
  // privacy-sensitive - a held-open mic keeps the OS "mic in use" indicator lit
  // the whole time. Only ever active while `dictation_enabled` is also true.
  dictation_warm_mic: {
    type: "boolean",
    path: ["general", "dictation_warm_mic"],
    default: false as boolean,
  },
  // How long delivered dictation history is kept before pruning: "off" (keep
  // forever - the default), "7d", "30d" or "90d". The retention/prune logic is
  // owned elsewhere; this only declares the setting.
  dictation_history_retention: {
    type: "string",
    path: ["general", "dictation_history_retention"],
    default: "off" as string,
  },
  // Second global hotkey (works while the app is backgrounded, all platforms):
  // pastes the most recent delivered dictation at the cursor. Empty string =
  // disabled (no default binding, so it can't collide with the toggle above out
  // of the box).
  dictation_paste_last_shortcut: {
    type: "string",
    path: ["general", "dictation_paste_last_shortcut"],
    default: "" as string,
  },
  // "type" (segments typed live into the focused app) or "batch" (accumulate;
  // delivered once on stop - terminal-friendly). The pre-rework value
  // "batch-paste" is tolerated and migrated to "batch" + paste-at-cursor on.
  dictation_output_mode: {
    type: "string",
    path: ["general", "dictation_output_mode"],
    default: "type" as string,
  },
  // Batch mode only: paste the transcript at the cursor on stop (true) or
  // copy it to the clipboard only (false - the user pastes manually).
  dictation_paste_at_cursor: {
    type: "boolean",
    path: ["general", "dictation_paste_at_cursor"],
    default: true as boolean,
  },
  // Transcript cleanup applied when a dictation finishes ("none" | "basic" |
  // "llm"). Applies to the batch-delivered text and to what history stores;
  // type mode always types raw segments live.
  dictation_cleanup: {
    type: "string",
    path: ["general", "dictation_cleanup"],
    default: "basic" as string,
  },
  // Translation mode for dictation cleanup. When enabled, the finalize LLM
  // pass translates the dictated speech into `dictation_translation_target`
  // instead of only cleaning it (still stripping fillers + fixing punctuation).
  // Requires an LLM to be configured; when unreachable it falls back to the
  // rule-cleaned SOURCE text so a paste is never blocked. target == source
  // effectively no-ops into normal cleanup.
  dictation_translation_enabled: {
    type: "boolean",
    path: ["general", "dictation_translation_enabled"],
    default: false as boolean,
  },
  // Target language for `dictation_translation_enabled` - a language code or
  // name understood by the LLM (e.g. "en", "English", "es"). Default "en";
  // the primary use case is mixed Hinglish speech -> English.
  dictation_translation_target: {
    type: "string",
    path: ["general", "dictation_translation_target"],
    default: "en" as string,
  },
  // Orb look: "cobalt-halo" (default; twin cobalt rings in a canvas bloom),
  // "cobalt" (the mini meeting orb), "particles" (voice-reactive particle
  // sphere), "waveform" ("Pulse", the dancing-sticks waveform), etc. Keep in
  // sync with `DEFAULT_ORB_VARIANT` in `dictation/orb.tsx`.
  dictation_orb_variant: {
    type: "string",
    path: ["general", "dictation_orb_variant"],
    default: "cobalt" as string,
  },
  floating_bar_opacity: {
    type: "number",
    path: ["general", "floating_bar_opacity"],
    default: 0.78 as number,
  },
  live_caption_opacity: {
    type: "number",
    path: ["general", "live_caption_opacity"],
    default: 0.3 as number,
  },
  live_caption_width: {
    type: "number",
    path: ["general", "live_caption_width"],
    default: 440 as number,
  },
  live_caption_line_count: {
    type: "number",
    path: ["general", "live_caption_line_count"],
    default: 1 as number,
  },
  live_caption_position: {
    type: "string",
    path: ["general", "live_caption_position"],
    default: "topCenter" as string,
  },
  live_caption_minimized: {
    type: "boolean",
    path: ["general", "live_caption_minimized"],
    default: true as boolean,
  },
  show_app_in_dock: {
    type: "boolean",
    path: ["general", "show_app_in_dock"],
    default: true as boolean,
  },
  show_tray_icon: {
    type: "boolean",
    path: ["general", "show_tray_icon"],
    default: true as boolean,
  },
  theme: {
    type: "string",
    path: ["general", "theme"],
    default: "system" as string,
  },
  save_recordings: {
    type: "boolean",
    path: ["general", "save_recordings"],
    default: true as boolean,
  },
  // Opt-in DTLN noise suppression applied to the transcription-bound mic
  // copy only (recordings stay raw). Read at session start; changing it does
  // not affect an in-flight session.
  mic_denoise: {
    type: "boolean",
    path: ["general", "mic_denoise"],
    default: false as boolean,
  },
  audio_retention: {
    type: "string",
    path: ["general", "audio_retention"],
    default: "forever" as string,
  },
  // When off, meetings skip real-time transcription — audio is still recorded
  // and the whole recording is transcribed in one batch pass when you stop.
  // Lighter for long meetings (no continuous STT + no long-lived websocket) and
  // avoids a live stream that can stall "stop". Read at session start; changing
  // it does not affect an in-flight session.
  live_transcription_enabled: {
    type: "boolean",
    path: ["general", "live_transcription_enabled"],
    default: true as boolean,
  },
  // Override for diarization's speaker count. Absent (undefined) = automatic —
  // the calibrated auto-count / threshold-based auto-clustering stays the
  // primary path. A positive integer forces exactly that many speakers
  // (feeds `options.numSpeakers` in useRunBatch; Rust treats Some(n>0) as a
  // fixed count and None as auto). Read at batch-run time.
  diarization_speaker_count: {
    type: "number",
    path: ["general", "diarization_speaker_count"],
  },
  notification_event: {
    type: "boolean",
    path: ["notification", "event"],
    default: true as boolean,
  },
  notification_detect: {
    type: "boolean",
    path: ["notification", "detect"],
    default: true as boolean,
  },
  respect_dnd: {
    type: "boolean",
    path: ["notification", "respect_dnd"],
    default: false as boolean,
  },
  telemetry_consent: {
    type: "boolean",
    path: ["general", "telemetry_consent"],
    default: true as boolean,
  },
  ai_language: {
    type: "string",
    path: ["language", "ai_language"],
    default: "en" as string,
  },
  spoken_languages: {
    type: "string",
    path: ["language", "spoken_languages"],
    default: "[]" as string,
  },
  personalization_dictionary_terms: {
    type: "string",
    path: ["personalization", "dictionary_terms"],
    default: "[]" as string,
  },
  custom_summary_instructions: {
    type: "string",
    path: ["personalization", "custom_summary_instructions"],
    default: "" as string,
  },
  ignored_platforms: {
    type: "string",
    path: ["notification", "ignored_platforms"],
    default: "[]" as string,
  },
  included_platforms: {
    type: "string",
    path: ["notification", "included_platforms"],
    default: "[]" as string,
  },
  mic_active_threshold: {
    type: "number",
    path: ["notification", "mic_active_threshold"],
    default: 15 as number,
  },
  current_llm_provider: {
    type: "string",
    path: ["ai", "current_llm_provider"],
  },
  // "My model is capable, stop second-guessing me": bypasses the llm-router
  // minimum-size heuristic for structured-output tasks (action items). The
  // structural verbatim-source gates still apply downstream.
  llm_caps_override: {
    type: "boolean",
    path: ["ai", "llm_caps_override"],
    default: false as boolean,
  },
  current_llm_model: {
    type: "string",
    path: ["ai", "current_llm_model"],
  },
  // Per-scope LLM overrides. Each scope (cleanup / notes / chat) may pin its
  // own provider + model; an empty string inherits the global selection
  // (`current_llm_provider` / `current_llm_model`). Resolution + the
  // cloud-opt-in invariant live in `~/ai/scope.ts` + `~/ai/hooks`; the picker
  // UI is owned separately. A cloud override is honoured ONLY when cloud is
  // already opted into globally - otherwise it silently inherits the global.
  ai_scope_cleanup_provider: {
    type: "string",
    path: ["ai", "scope_cleanup_provider"],
    default: "" as string,
  },
  ai_scope_cleanup_model: {
    type: "string",
    path: ["ai", "scope_cleanup_model"],
    default: "" as string,
  },
  ai_scope_notes_provider: {
    type: "string",
    path: ["ai", "scope_notes_provider"],
    default: "" as string,
  },
  ai_scope_notes_model: {
    type: "string",
    path: ["ai", "scope_notes_model"],
    default: "" as string,
  },
  ai_scope_chat_provider: {
    type: "string",
    path: ["ai", "scope_chat_provider"],
    default: "" as string,
  },
  ai_scope_chat_model: {
    type: "string",
    path: ["ai", "scope_chat_model"],
    default: "" as string,
  },
  current_stt_provider: {
    type: "string",
    path: ["ai", "current_stt_provider"],
  },
  current_stt_model: {
    type: "string",
    path: ["ai", "current_stt_model"],
  },
  // Model used for the post-meeting ("final") transcription pass and manual
  // re-transcription. Empty string = use the live model (current_stt_model).
  final_stt_model: {
    type: "string",
    path: ["ai", "final_stt_model"],
    default: "" as string,
  },
  // Provider for the batch/final-pass model. Empty/undefined = use the live
  // provider (current_stt_provider), i.e. the pre-split behavior. Set to a
  // batch-capable provider id ("custom", "deepgram", "hyprnote", …) to route
  // batch transcription independently of the local-only live selection.
  final_stt_provider: {
    type: "string",
    path: ["ai", "final_stt_provider"],
    default: "" as string,
  },
  timezone: {
    type: "string",
    path: ["general", "timezone"],
  },
  week_start: {
    type: "string",
    path: ["general", "week_start"],
  },
  selected_template_id: {
    type: "string",
    path: ["general", "selected_template_id"],
  },
  todo_linear_filter: {
    type: "string",
    path: ["todo", "linear_filter"],
    default: "" as string,
  },
  todo_github_repository: {
    type: "string",
    path: ["todo", "github_repository"],
    default: "" as string,
  },
  // Live caption bubble above the dictation orb: the last few recognized
  // words, fading out when you pause (Windows/Linux orb path only).
  dictation_caption: {
    type: "boolean",
    path: ["general", "dictation_caption"],
    default: true as boolean,
  },
  // Runtime opt-in for P2P device sync (experimental). Compiling the `sync`
  // feature in no longer starts it — this must also be true. Default OFF:
  // enabling it starts a background agent that, in `Discovered` transport
  // mode (the only mode the app uses), publishes this device's node id and
  // reachable addresses to n0.computer's DNS/pkarr discovery infrastructure
  // for as long as it runs (docs/internal/sync-p2p.md §20.3). Read on the
  // Rust side at startup straight from `app_settings`; a missing/unreadable
  // row must default to false, never panic.
  sync_enabled: {
    type: "boolean",
    path: ["sync", "enabled"],
    default: false as boolean,
  },
} as const;

export type SettingKey = keyof typeof SETTING_DEFINITIONS;

type SettingTypeMap = {
  boolean: boolean;
  number: number;
  string: string;
};

export type SettingValue<K extends SettingKey> =
  SettingTypeMap[(typeof SETTING_DEFINITIONS)[K]["type"]];

export type SettingValues = {
  [K in SettingKey]?: SettingValue<K>;
};

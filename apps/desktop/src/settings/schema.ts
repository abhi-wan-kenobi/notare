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
  // Orb look: "cobalt" (the mini meeting orb), "particles" (voice-reactive
  // particle sphere) or "waveform" ("Pulse", the dancing-sticks waveform).
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
  current_llm_model: {
    type: "string",
    path: ["ai", "current_llm_model"],
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

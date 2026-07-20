import type { LocalModel } from "@hypr/plugin-local-stt";
import {
  commands as listenerCommands,
  type TranscriptionMode,
} from "@hypr/plugin-transcription";
import { HINGLISH_LANGUAGE_CODE } from "~/settings/general/language";

type LiveTranscriptionConfig = {
  languages: string[];
  transcriptionMode?: TranscriptionMode;
};

const SONIQO_PARAKEET_BATCH_LANGUAGE_CODES = new Set([
  "bg",
  "cs",
  "da",
  "de",
  "el",
  "en",
  "es",
  "et",
  "fi",
  "fr",
  "hr",
  "hu",
  "it",
  "lt",
  "lv",
  "mt",
  "nl",
  "pl",
  "pt",
  "ro",
  "ru",
  "sk",
  "sl",
  "sv",
  "uk",
]);
const SONIQO_STREAMING_LANGUAGE_CODES = SONIQO_PARAKEET_BATCH_LANGUAGE_CODES;

export function isSupportedLocalSttModel(
  model?: string | null,
): model is LocalModel {
  return (
    typeof model === "string" &&
    (model.startsWith("soniqo-") ||
      model.startsWith("am-") ||
      model.startsWith("parakeet-") ||
      model.startsWith("voxtral-") ||
      model.startsWith("Quantized"))
  );
}

export function isWhisperLocalSttModel(model?: string | null) {
  return typeof model === "string" && model.startsWith("Quantized");
}

export function isParakeetLocalSttModel(model?: string | null) {
  return typeof model === "string" && model.startsWith("parakeet-");
}

export function isVoxtralLocalSttModel(model?: string | null) {
  return typeof model === "string" && model.startsWith("voxtral-");
}

export function isHyprnoteCloudSttModel(
  provider?: string | null,
  model?: string | null,
) {
  return provider === "hyprnote" && model === "cloud";
}

export function isHyprnoteLocalSttModel(
  provider?: string | null,
  model?: string | null,
): model is LocalModel {
  return provider === "hyprnote" && isSupportedLocalSttModel(model);
}

export function isConfiguredSttModel(
  provider?: string | null,
  model?: string | null,
) {
  if (!provider || !model) {
    return false;
  }

  if (provider === "hyprnote") {
    return model === "cloud" || isSupportedLocalSttModel(model);
  }

  return true;
}

export function isRealtimeLocalModel(model?: string | null) {
  return model === "soniqo-parakeet-streaming";
}

function baseLanguageCode(language: string) {
  return language.split(/[-_]/)[0]?.toLowerCase() ?? "";
}

function languageSupportProvider(provider: string) {
  return provider === "custom" || provider === "cloudflare_workers_ai"
    ? "deepgram"
    : provider;
}

export async function isSupportedLanguagesLive(
  provider: string,
  model: string | null | undefined,
  languages: readonly string[],
) {
  const result = await listenerCommands.isSupportedLanguagesLive(
    languageSupportProvider(provider),
    model ?? null,
    [...languages],
  );

  return result.status === "ok" ? result.data : true;
}

export async function isSupportedLanguagesBatch(
  provider: string,
  model: string | null | undefined,
  languages: readonly string[],
) {
  const result = await listenerCommands.isSupportedLanguagesBatch(
    languageSupportProvider(provider),
    model ?? null,
    [...languages],
  );

  return result.status === "ok" ? result.data : true;
}

export function getTranscriptionLanguages(
  mainLanguage: string | null | undefined,
  spokenLanguages: readonly string[] | null | undefined,
) {
  const seen = new Set<string>();
  const languages: string[] = [];

  for (const language of [mainLanguage, ...(spokenLanguages ?? [])]) {
    if (!language) {
      continue;
    }

    const baseCode = baseLanguageCode(language);
    if (!baseCode || seen.has(baseCode)) {
      continue;
    }

    seen.add(baseCode);
    languages.push(language);
  }

  return languages;
}

/// Expand the Hinglish sentinel into real language codes for the active engine.
///
/// Hinglish (Hindi-English code-mix) has no single ISO code. Voxtral is an LLM
/// with a promptable Hinglish mode, so it gets `hi,en` (its engine turns that
/// into romanized code-mix). Every other engine — bundled Whisper (`Quantized`)
/// or a remote custom Whisper server — has no romanized-Hinglish mode, so we
/// ask for `en`, Whisper's least-bad code-mix behavior. See issue #40.
///
/// A no-op when the sentinel isn't present. Result is de-duplicated.
export function expandHinglish(
  languages: string[],
  engine: { provider?: string | null; model?: string | null },
): string[] {
  if (!languages.includes(HINGLISH_LANGUAGE_CODE)) {
    return languages;
  }

  const replacement = isVoxtralLocalSttModel(engine.model)
    ? ["hi", "en"]
    : ["en"];

  const out: string[] = [];
  for (const language of languages) {
    const mapped = language === HINGLISH_LANGUAGE_CODE ? replacement : [language];
    for (const code of mapped) {
      if (!out.includes(code)) {
        out.push(code);
      }
    }
  }
  return out;
}

export function getOnDeviceTranscriptionConfig(
  model: string | null | undefined,
  languages: readonly string[],
): LiveTranscriptionConfig {
  if (isWhisperLocalSttModel(model) || isParakeetLocalSttModel(model)) {
    // The internal whisper/parakeet server streams VAD-chunked final
    // transcripts over the /v1/listen websocket, so these models support
    // live transcription on every platform (falls back to batch
    // automatically if the listener fails to connect).
    return {
      languages: [...languages],
      transcriptionMode: "live",
    };
  }

  if (!isRealtimeLocalModel(model)) {
    return {
      languages: [...languages],
      transcriptionMode: "batch",
    };
  }

  const supportedLiveLanguages = languages.filter((language) =>
    SONIQO_STREAMING_LANGUAGE_CODES.has(baseLanguageCode(language)),
  );

  if (languages.length > 0 && supportedLiveLanguages.length === 0) {
    return {
      languages: [],
      transcriptionMode: "live",
    };
  }

  return {
    languages:
      supportedLiveLanguages.length > 0
        ? [supportedLiveLanguages[0]]
        : [...languages],
    transcriptionMode: "live",
  };
}

export function getOnDeviceTranscriptionMode(
  model: string | null | undefined,
  languages: readonly string[] = [],
) {
  return getOnDeviceTranscriptionConfig(model, languages).transcriptionMode;
}

export async function getLiveTranscriptionConfig({
  provider,
  model,
  languages,
}: {
  provider?: string | null;
  model?: string | null;
  languages: readonly string[];
}): Promise<LiveTranscriptionConfig> {
  if (isHyprnoteLocalSttModel(provider, model)) {
    return getOnDeviceTranscriptionConfig(model, languages);
  }

  const config = {
    languages: [...languages],
    transcriptionMode: undefined as TranscriptionMode | undefined,
  } satisfies LiveTranscriptionConfig;

  if (!provider || languages.length <= 1) {
    return config;
  }

  if (await isSupportedLanguagesLive(provider, model, languages)) {
    return config;
  }

  const primaryLanguage = languages[0];
  if (
    primaryLanguage &&
    (await isSupportedLanguagesLive(provider, model, [primaryLanguage]))
  ) {
    return {
      ...config,
      languages: [primaryLanguage],
    };
  }

  return config;
}

export async function isLiveTranscriptionSupported(
  provider?: string | null,
  model?: string | null,
) {
  if (!provider || !model) {
    return false;
  }

  // Live transcription runs on the on-device loopback /v1/listen server, so
  // only local ("hyprnote") models can do it. Remote/custom/cloud providers
  // are batch-only — never report them as live-capable, even if they speak a
  // Deepgram-compatible protocol (those go through the batch adapter).
  if (!isHyprnoteLocalSttModel(provider, model)) {
    return false;
  }

  return isSupportedLanguagesLive(provider, model, []);
}

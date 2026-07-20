import { platform } from "@tauri-apps/plugin-os";
import { useCallback } from "react";

import type { TranscriptionParams } from "@hypr/plugin-transcription";
import { commands as transcriptionCommands } from "@hypr/plugin-transcription";
import { sonnerToast } from "@hypr/ui/components/ui/toast";

import { useListener } from "./contexts";
import { pickFinalSttModel, resolveFinalBatchTarget } from "./final-model";
import { getSessionKeywords } from "./useKeywords";
import { useSTTConnection } from "./useSTTConnection";

import { useAuth } from "~/auth";
import { useBillingAccess } from "~/auth/billing";
import { env } from "~/env";
import {
  deleteProcessedAudioForRetention,
  normalizeAudioRetention,
} from "~/services/audio-retention";
import { useSession, useSessionParticipants } from "~/session/queries";
import { useAiProvider } from "~/settings/providers";
import { useConfigValue } from "~/shared/config";
import { id } from "~/shared/utils";
import type { BatchPersistCallback } from "~/store/zustand/listener/transcript";
import {
  getTranscriptionLanguages,
  isHyprnoteLocalSttModel,
  isParakeetLocalSttModel,
  isSupportedLanguagesBatch,
  isWhisperLocalSttModel,
} from "~/stt/capabilities";
import { appendTranscriptWordsAndHints, createTranscript } from "~/stt/queries";
import type { SpeakerHintWithId, WordWithId } from "~/stt/types";

type RunOptions = {
  handlePersist?: BatchPersistCallback;
  model?: string;
  /**
   * Provider the explicit `model` belongs to (e.g. "hyprnote" for the
   * "Re-transcribe with…" picker). Falls back to the live connection's
   * provider.
   */
  provider?: string;
  baseUrl?: string;
  apiKey?: string;
  keywords?: string[];
  languages?: string[];
  numSpeakers?: number;
  minSpeakers?: number;
  maxSpeakers?: number;
};

type BatchTarget = {
  provider: TranscriptionParams["provider"];
  model: string;
  baseUrl: string;
  apiKey: string;
  label: string;
};

const DIRECT_BATCH_PROVIDERS: Set<TranscriptionParams["provider"]> = new Set([
  "deepgram",
  "cartesia",
  "soniox",
  "assemblyai",
  "openai",
  "gladia",
  "elevenlabs",
  "mistral",
  "fireworks",
  "pyannote",
  "aquavoice",
]);

export const STOPPED_TRANSCRIPTION_ERROR_MESSAGE = "Transcription stopped.";
const LOCAL_SONIQO_BATCH_TARGET = {
  provider: "soniqo",
  model: "soniqo-parakeet-batch",
  baseUrl: "soniqo://local",
  apiKey: "",
  label: "Soniqo batch transcription",
} satisfies BatchTarget;

export function getBatchProvider(
  provider: string,
  model: string,
): TranscriptionParams["provider"] | null {
  if (provider === "cloudflare_workers_ai") {
    return "deepgram";
  }

  if (provider === "hyprnote") {
    if (model.startsWith("soniqo-")) return "soniqo";
    if (model.startsWith("am-")) return "am";
    return "hyprnote";
  }

  // A "custom" provider points at a self-hosted companion server that speaks
  // the same /v1/listen protocol as the local hyprnote server, so it routes
  // through the hyprnote batch adapter (carrying the custom base URL/API key
  // on the BatchTarget).
  if (provider === "custom") {
    return "hyprnote";
  }

  if (DIRECT_BATCH_PROVIDERS.has(provider as TranscriptionParams["provider"])) {
    return provider as TranscriptionParams["provider"];
  }
  return null;
}

export function canRunBatchTranscription(
  _conn: { provider: string; model: string } | null,
  _modelOverride?: string,
) {
  return true;
}

export const NO_BATCH_PROVIDER_ERROR =
  "No batch transcription provider is available. Configure a speech-to-text provider in Settings.";

/**
 * Builds a local batch fallback from the live connection when it is a
 * downloaded local whisper/parakeet model — those servers handle batch on
 * every platform. Returns null when the live model can't serve batch (e.g.
 * a macOS-only streaming model, or a non-local connection).
 */
export function getLocalBatchFallbackFromConn(
  conn: { provider: string; model: string; baseUrl: string } | null,
): BatchTarget | null {
  if (!conn || !isHyprnoteLocalSttModel(conn.provider, conn.model)) {
    return null;
  }

  if (
    isWhisperLocalSttModel(conn.model) ||
    isParakeetLocalSttModel(conn.model)
  ) {
    return {
      provider: "hyprnote",
      model: conn.model,
      baseUrl: conn.baseUrl,
      apiKey: "",
      label: "Local transcription",
    };
  }

  return null;
}

/**
 * Picks the safety-net batch target when the selected target can't be used.
 *
 * - Paid users with a session: hosted cloud transcription.
 * - macOS: the Soniqo batch target (Apple-Silicon-only backend).
 * - Windows/Linux: a downloaded local whisper/parakeet batch model from the
 *   live connection, when one is available. Returns null when there is none
 *   — callers must surface a clear error rather than silently no-op'ing.
 */
export function getBatchFallbackTarget({
  isPaid,
  accessToken,
  apiBaseUrl,
  platform = "macos",
  localBatchFallback,
}: {
  isPaid: boolean;
  accessToken?: string | null;
  apiBaseUrl: string;
  platform?: string;
  localBatchFallback?: BatchTarget | null;
}): BatchTarget | null {
  if (isPaid && accessToken) {
    return {
      provider: "hyprnote",
      model: "cloud",
      baseUrl: new URL("/stt", apiBaseUrl).toString(),
      apiKey: accessToken,
      label: "Pro cloud transcription",
    };
  }

  if (platform === "macos") {
    return LOCAL_SONIQO_BATCH_TARGET;
  }

  return localBatchFallback ?? null;
}

async function canUseBatchTarget(
  provider: TranscriptionParams["provider"],
  model: string,
  languages: readonly string[],
) {
  return isSupportedLanguagesBatch(provider, model, languages);
}

function selectedProviderLabel(
  conn: { provider: string; model: string } | null,
  modelOverride?: string,
) {
  if (!conn) {
    return "the selected speech-to-text provider";
  }

  return modelOverride ?? conn.model ?? conn.provider;
}

function sameBatchTarget(
  a: Pick<BatchTarget, "provider" | "model"> | null,
  b: Pick<BatchTarget, "provider" | "model"> | null,
) {
  return a?.provider === b?.provider && a?.model === b?.model;
}

export function isStoppedTranscriptionError(error: unknown) {
  return (
    (error instanceof Error ? error.message : String(error)) ===
    STOPPED_TRANSCRIPTION_ERROR_MESSAGE
  );
}

export function getSessionSpeakerCount(
  participantHumanIds: Iterable<string>,
  selfHumanId?: string | null,
): number | undefined {
  const humanIds = new Set(
    Array.from(participantHumanIds).filter((humanId) => Boolean(humanId)),
  );

  if (typeof selfHumanId === "string" && selfHumanId) {
    humanIds.add(selfHumanId);
  }

  return humanIds.size > 1 ? humanIds.size : undefined;
}

export const useRunBatch = (sessionId: string) => {
  const session = useSession(sessionId);
  const participants = useSessionParticipants(sessionId);

  const startTranscription = useListener((state) => state.startTranscription);
  const { conn } = useSTTConnection();
  const auth = useAuth();
  const billing = useBillingAccess();
  const aiLanguage = useConfigValue("ai_language");
  const spokenLanguages = useConfigValue("spoken_languages");
  const dictionaryTerms = useConfigValue("personalization_dictionary_terms");
  const finalSttModel = useConfigValue("final_stt_model");
  const finalSttProvider = useConfigValue("final_stt_provider");
  // Batch may run on a different provider than the local-only live
  // connection (e.g. a custom companion server or cloud). Resolve the final
  // provider's configured base URL/API key so the BatchTarget carries it.
  const finalProviderConfig = useAiProvider(
    "stt",
    finalSttProvider || undefined,
  );
  const audioRetention = normalizeAudioRetention(
    useConfigValue("audio_retention"),
  );

  return useCallback(
    async (filePath: string, options?: RunOptions) => {
      if (!startTranscription) {
        throw new Error(
          "STT connection is not available. Please configure your speech-to-text provider.",
        );
      }

      // Post-meeting passes prefer the configured final model; an explicit
      // options.model always wins, and everything falls back to the live
      // model (today's behavior) when no usable final model is configured.
      // Batch may target a different provider than the live connection when
      // `final_stt_provider` is set (e.g. custom server / cloud); otherwise it
      // reuses the live provider.
      const batchProviderSetting =
        typeof finalSttProvider === "string" ? finalSttProvider.trim() : "";
      const effectiveBatchProvider = batchProviderSetting || conn?.provider;
      const finalModelPick =
        options?.model === undefined
          ? pickFinalSttModel({
              finalSetting: finalSttModel,
              liveModel: conn?.model,
            })
          : null;
      const finalTarget =
        finalModelPick && effectiveBatchProvider
          ? await resolveFinalBatchTarget({
              provider: effectiveBatchProvider,
              liveModel: conn?.model,
              finalModel: finalModelPick,
              finalBaseUrl: batchProviderSetting
                ? finalProviderConfig?.base_url?.trim() || undefined
                : undefined,
              finalApiKey: batchProviderSetting
                ? finalProviderConfig?.api_key?.trim() || undefined
                : undefined,
            })
          : null;

      const languages =
        options?.languages ??
        getTranscriptionLanguages(aiLanguage, spokenLanguages);
      const selectedModel = options?.model ?? finalTarget?.model ?? conn?.model;
      const selectionProvider = options?.provider ?? effectiveBatchProvider;
      const selectedProvider =
        selectionProvider && selectedModel
          ? getBatchProvider(selectionProvider, selectedModel)
          : null;
      const selectedBaseUrl =
        options?.baseUrl ?? finalTarget?.baseUrl ?? conn?.baseUrl;
      const selectedTarget =
        selectedModel && selectedProvider && selectedBaseUrl !== undefined
          ? {
              provider: selectedProvider,
              model: selectedModel,
              baseUrl: selectedBaseUrl,
              apiKey:
                options?.apiKey ?? finalTarget?.apiKey ?? conn?.apiKey ?? "",
              label: selectedModel,
            }
          : null;
      const selectedTargetSupported = selectedTarget
        ? await canUseBatchTarget(
            selectedTarget.provider,
            selectedTarget.model,
            languages,
          )
        : false;
      const fallbackTarget = getBatchFallbackTarget({
        isPaid: billing.isPaid,
        accessToken: auth?.session?.access_token,
        apiBaseUrl: env.VITE_API_URL,
        platform: platform(),
        localBatchFallback: getLocalBatchFallbackFromConn(conn),
      });
      const shouldUseSelectedTarget =
        selectedTargetSupported ||
        sameBatchTarget(selectedTarget, fallbackTarget);
      const target = shouldUseSelectedTarget
        ? (selectedTarget ?? fallbackTarget)
        : fallbackTarget;

      if (!target) {
        throw new Error(NO_BATCH_PROVIDER_ERROR);
      }

      if (!shouldUseSelectedTarget) {
        sonnerToast.message("Using a batch transcription provider", {
          description: `${
            selectedTarget
              ? selectedProviderLabel(conn, selectedModel)
              : selectedProviderLabel(conn)
          } is not available for batch transcription. Using ${target.label} instead.`,
        });
      }

      const createdAt = new Date().toISOString();
      const memoMd = session?.raw_md ?? "";
      const keywords =
        options?.keywords ??
        (await getSessionKeywords({
          sessionId,
          dictionaryTerms,
        }));
      let transcriptId: string | null = null;
      // Final word set (id + timing) captured from persist so a local
      // diarization pass can label them by speaker after transcription.
      let diarizationWords: { id: string; start_ms: number; end_ms: number }[] =
        [];
      const inferredNumSpeakers =
        options?.numSpeakers === undefined &&
        options?.minSpeakers === undefined &&
        options?.maxSpeakers === undefined
          ? getSessionSpeakerCount(
              participants
                .filter((participant) => participant.source !== "excluded")
                .map((participant) => participant.humanId),
              session?.user_id,
            )
          : undefined;

      const handlePersist: BatchPersistCallback | undefined =
        options?.handlePersist;
      let lastTranscriptWrite = Promise.resolve();
      let transcriptWriteError: unknown;
      const trackTranscriptWrite = (write: Promise<void>) => {
        lastTranscriptWrite = write.catch((error) => {
          transcriptWriteError = error;
          console.error("[runBatch] failed to persist transcript", error);
        });
      };

      const persist =
        handlePersist ??
        ((words, hints, persistOptions) => {
          if (words.length === 0) {
            return;
          }

          const newWords: WordWithId[] = [];
          const newWordIds: string[] = [];

          words.forEach((word) => {
            const wordId = id();

            newWords.push({
              id: wordId,
              text: word.text,
              start_ms: word.start_ms,
              end_ms: word.end_ms,
              channel: word.channel,
              metadata: word.metadata
                ? JSON.stringify(word.metadata)
                : undefined,
            });

            newWordIds.push(wordId);
          });

          // Batch persists in "replace" mode (last call = the full transcript),
          // so replace; append only if a custom persist path asks for it.
          // Words without timing can't be aligned to a speaker span — skip them.
          const captured: { id: string; start_ms: number; end_ms: number }[] =
            [];
          for (const w of newWords) {
            if (
              typeof w.start_ms === "number" &&
              typeof w.end_ms === "number"
            ) {
              captured.push({
                id: w.id,
                start_ms: w.start_ms,
                end_ms: w.end_ms,
              });
            }
          }
          diarizationWords =
            persistOptions?.mode === "append"
              ? [...diarizationWords, ...captured]
              : captured;

          const newHints: SpeakerHintWithId[] = [];

          hints.forEach((hint) => {
            if (hint.data.type !== "provider_speaker_index") {
              return;
            }

            const wordId = newWordIds[hint.wordIndex];
            const word = words[hint.wordIndex];

            if (!wordId || !word) {
              return;
            }

            newHints.push({
              id: id(),
              word_id: wordId,
              type: "provider_speaker_index",
              value: JSON.stringify({
                provider: hint.data.provider ?? target.provider,
                channel: hint.data.channel ?? word.channel,
                speaker_index: hint.data.speaker_index,
              }),
            });
          });

          if (!transcriptId) {
            transcriptId = id();
            trackTranscriptWrite(
              createTranscript({
                id: transcriptId,
                sessionId,
                ownerUserId: session?.user_id ?? "",
                createdAt,
                startedAt: Date.now(),
                memo: memoMd,
                source: "batch_transcription",
                provider: target.provider,
                model: target.model,
                words: newWords,
                speakerHints: newHints,
                replaceSession: true,
              }),
            );
          } else {
            trackTranscriptWrite(
              appendTranscriptWordsAndHints(
                transcriptId,
                newWords,
                newHints,
                persistOptions,
              ),
            );
          }
        });

      const params: TranscriptionParams = {
        session_id: sessionId,
        provider: target.provider,
        file_path: filePath,
        model: target.model,
        base_url: target.baseUrl,
        api_key: target.apiKey,
        keywords,
        languages,
        num_speakers: options?.numSpeakers ?? inferredNumSpeakers,
        min_speakers: options?.minSpeakers,
        max_speakers: options?.maxSpeakers,
      };

      try {
        await startTranscription(params, { handlePersist: persist });
      } finally {
        await lastTranscriptWrite;
        // Bring the live model's local server back if the final pass swapped
        // it out (no-op for external providers / unset final model).
        await finalTarget?.restore();
      }

      if (transcriptWriteError) throw transcriptWriteError;

      // On-device speaker diarization for local engines (Whisper/Parakeet emit
      // no speakers; cloud providers already diarize). Best-effort — never fail
      // the transcription over it — and run before retention deletes the audio.
      if (
        transcriptId &&
        diarizationWords.length > 0 &&
        (isWhisperLocalSttModel(target.model) ||
          isParakeetLocalSttModel(target.model))
      ) {
        try {
          const diarized = await transcriptionCommands.runDiarization(
            filePath,
            params.num_speakers ?? null,
            diarizationWords,
            // Enrolled voice profiles for recognition — wired to the
            // voice_profiles store + enrollment UX in P2.6 (#15). Empty for now
            // means diarization-only ("Speaker N", no auto-naming).
            [],
          );
          if (
            diarized.status === "ok" &&
            diarized.data.word_speakers.length > 0
          ) {
            const speakerHints: SpeakerHintWithId[] =
              diarized.data.word_speakers.map((r) => ({
                id: id(),
                word_id: r.word_id,
                type: "provider_speaker_index",
                value: JSON.stringify({
                  provider: target.provider,
                  speaker_index: r.speaker_index,
                }),
              }));
            await appendTranscriptWordsAndHints(transcriptId, [], speakerHints);
          }
        } catch (error) {
          console.error("[runBatch] on-device diarization failed", error);
        }
      }

      await deleteProcessedAudioForRetention(audioRetention, sessionId);
    },
    [
      conn,
      auth?.session?.access_token,
      aiLanguage,
      audioRetention,
      billing.isPaid,
      dictionaryTerms,
      finalSttModel,
      finalSttProvider,
      finalProviderConfig,
      session,
      participants,
      spokenLanguages,
      startTranscription,
      sessionId,
    ],
  );
};

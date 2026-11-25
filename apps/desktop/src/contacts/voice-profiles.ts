import type {
  AudioSpanMs,
  DiarWordSpeaker,
  EnrolledProfile,
  SpeakerHuman,
} from "@hypr/plugin-transcription";
import { commands as transcriptionCommands } from "@hypr/plugin-transcription";

import { executeTransaction, liveQueryClient, useLiveQuery } from "~/db";
import { enqueueDatabaseWrite } from "~/db/write-queue";
import { id } from "~/shared/utils";
import type { SpeakerHintWithId } from "~/stt/types";

// Embeddings come from the embedding model bundled in plugins/transcription.
// Profiles from another model are not comparable, so each row is tagged and
// only matching rows are used for recognition.
export const VOICE_EMBEDDING_MODEL = "bundled-3dspeaker-v1";

// The speaker-embedding model needs a few seconds of speech to be reliable;
// more than a minute adds compute without improving the profile.
export const MIN_ENROLLMENT_SPEECH_MS = 3_000;
const MAX_ENROLLMENT_SPEECH_MS = 60_000;
// Adjacent words closer than this are one continuous stretch of speech.
const MAX_WORD_GAP_MS = 400;

export type VoiceProfileSummary = { id: string; updatedAt: string };

// The JS→SQLite bridge binds arrays as JSON text, never as BLOBs, so the
// embedding crosses as hex and SQLite's unhex()/hex() convert at the edge.
// Layout matches crates/db-app voice_profile_ops: f32 little-endian.
export function encodeEmbeddingHex(embedding: number[]): string {
  const view = new DataView(new ArrayBuffer(embedding.length * 4));
  embedding.forEach((value, index) => view.setFloat32(index * 4, value, true));
  let hex = "";
  for (let offset = 0; offset < view.byteLength; offset += 1) {
    hex += view.getUint8(offset).toString(16).padStart(2, "0");
  }
  return hex;
}

export function decodeEmbeddingHex(hex: string): number[] {
  if (hex.length === 0 || hex.length % 8 !== 0 || !/^[0-9a-f]+$/i.test(hex)) {
    throw new Error("voice embedding is not a whole number of f32 values");
  }
  const view = new DataView(new ArrayBuffer(hex.length / 2));
  for (let i = 0; i < hex.length; i += 2) {
    view.setUint8(i / 2, parseInt(hex.slice(i, i + 2), 16));
  }
  const values: number[] = [];
  for (let offset = 0; offset < view.byteLength; offset += 4) {
    values.push(view.getFloat32(offset, true));
  }
  return values;
}

/**
 * Turn a speaker's words into audio spans for enrollment: contiguous speech is
 * merged, the longest stretches are kept up to a minute, and the result is in
 * recording order. `offsetMs` maps transcript time to recording time (non-zero
 * for a later transcript in a multi-part session).
 */
export function collectVoiceSpans(
  words: Array<{ start_ms?: number | null; end_ms?: number | null }>,
  offsetMs: number,
): { spans: AudioSpanMs[]; speechMs: number } {
  const timed = words
    .filter(
      (word): word is { start_ms: number; end_ms: number } =>
        typeof word.start_ms === "number" &&
        typeof word.end_ms === "number" &&
        word.end_ms > word.start_ms,
    )
    .map((word) => ({
      start: Math.max(0, Math.round(offsetMs + word.start_ms)),
      end: Math.max(0, Math.round(offsetMs + word.end_ms)),
    }))
    .sort((a, b) => a.start - b.start);

  const merged: Array<{ start: number; end: number }> = [];
  for (const word of timed) {
    const last = merged[merged.length - 1];
    if (last && word.start - last.end <= MAX_WORD_GAP_MS) {
      last.end = Math.max(last.end, word.end);
    } else {
      merged.push({ ...word });
    }
  }

  const kept: Array<{ start: number; end: number }> = [];
  let speechMs = 0;
  for (const span of [...merged].sort(
    (a, b) => b.end - b.start - (a.end - a.start),
  )) {
    if (speechMs >= MAX_ENROLLMENT_SPEECH_MS) break;
    const end = Math.min(
      span.end,
      span.start + MAX_ENROLLMENT_SPEECH_MS - speechMs,
    );
    kept.push({ start: span.start, end });
    speechMs += end - span.start;
  }

  return {
    spans: kept
      .sort((a, b) => a.start - b.start)
      .map((span) => ({ start_ms: span.start, end_ms: span.end })),
    speechMs,
  };
}

type VoiceWord = {
  id?: string | null;
  start_ms?: number | null;
  end_ms?: number | null;
};

/**
 * The words to learn a voice from after a speaker is assigned to a person.
 * "Apply to all" on a diarized speaker uses every word of that speaker on that
 * channel; otherwise only the segment's own words, because an undiarized
 * channel can hold several people.
 */
export function selectSpeakerWords(
  transcript: {
    words: Array<VoiceWord & { channel?: number | null }>;
    speakerHints: Array<{ word_id?: string; type?: string; value?: unknown }>;
  },
  segment: {
    key: { channel: string; speaker_index?: number | null };
    words: VoiceWord[];
  },
  mode: "all" | "segment",
): VoiceWord[] {
  const speakerIndex = segment.key.speaker_index;
  if (mode === "segment" || typeof speakerIndex !== "number") {
    return segment.words;
  }

  const channel =
    segment.key.channel === "DirectMic"
      ? 0
      : segment.key.channel === "RemoteParty"
        ? 1
        : 2;
  const speakerWordIds = new Set<string>();
  for (const hint of transcript.speakerHints) {
    if (hint.type !== "provider_speaker_index" || !hint.word_id) continue;
    let value = hint.value;
    if (typeof value === "string") {
      try {
        value = JSON.parse(value);
      } catch {
        continue;
      }
    }
    if (
      value &&
      typeof value === "object" &&
      "speaker_index" in value &&
      value.speaker_index === speakerIndex
    ) {
      speakerWordIds.add(hint.word_id);
    }
  }

  return transcript.words.filter(
    (word) =>
      typeof word.id === "string" &&
      speakerWordIds.has(word.id) &&
      (word.channel ?? 0) === channel,
  );
}

/**
 * Hints that name the diarized speakers recognized from enrolled voices. Each
 * is anchored on the speaker's first word without a `scope`, which the render
 * path reads as "every word of this speaker index on this channel" — the same
 * shape the speaker popover writes for "Apply to all".
 */
export function buildRecognizedSpeakerHints(
  wordSpeakers: DiarWordSpeaker[],
  speakerHumans: SpeakerHuman[],
): SpeakerHintWithId[] {
  const firstWordBySpeaker = new Map<number, string>();
  for (const { word_id, speaker_index } of wordSpeakers) {
    if (!firstWordBySpeaker.has(speaker_index)) {
      firstWordBySpeaker.set(speaker_index, word_id);
    }
  }

  return speakerHumans.flatMap(({ speaker_index, human_id }) => {
    const anchorWordId = firstWordBySpeaker.get(speaker_index);
    return anchorWordId
      ? [
          {
            id: `${anchorWordId}:user_speaker_assignment`,
            word_id: anchorWordId,
            type: "user_speaker_assignment",
            value: JSON.stringify({ human_id }),
          },
        ]
      : [];
  });
}

export async function loadEnrolledVoiceProfiles(): Promise<EnrolledProfile[]> {
  const rows = await liveQueryClient.execute<{
    human_id: string;
    embedding_hex: string;
  }>(
    `
      SELECT profile.human_id, hex(profile.embedding) AS embedding_hex
      FROM voice_profiles AS profile
      JOIN humans AS human
        ON human.id = profile.human_id AND human.deleted_at IS NULL
      WHERE profile.deleted_at IS NULL AND profile.model = ?
      ORDER BY profile.updated_at DESC, profile.id
    `,
    [VOICE_EMBEDDING_MODEL],
  );

  const profiles = new Map<string, EnrolledProfile>();
  for (const row of rows) {
    if (profiles.has(row.human_id)) continue;
    try {
      profiles.set(row.human_id, {
        human_id: row.human_id,
        embedding: decodeEmbeddingHex(row.embedding_hex),
      });
    } catch (error) {
      console.error("[voice-profiles] skipping unreadable profile", error);
    }
  }
  return [...profiles.values()];
}

export function useHumanVoiceProfile(
  humanId: string,
): VoiceProfileSummary | null {
  const { data = null } = useLiveQuery<
    { id: string; updated_at: string },
    VoiceProfileSummary | null
  >({
    sql: `
      SELECT id, updated_at
      FROM voice_profiles
      WHERE human_id = ? AND deleted_at IS NULL AND model = ?
      ORDER BY updated_at DESC, id
      LIMIT 1
    `,
    params: [humanId, VOICE_EMBEDDING_MODEL],
    enabled: Boolean(humanId),
    mapRows: (rows) =>
      rows[0] ? { id: rows[0].id, updatedAt: rows[0].updated_at } : null,
  });
  return humanId ? data : null;
}

/**
 * Compute a voice profile from parts of a recording and make it this person's
 * only profile. An empty `spans` list embeds the whole file (an imported clip
 * of just that person).
 */
export async function enrollVoice({
  humanId,
  audioPath,
  spans,
}: {
  humanId: string;
  audioPath: string;
  spans: AudioSpanMs[];
}): Promise<void> {
  const result = await transcriptionCommands.computeVoiceEmbedding(
    audioPath,
    spans,
  );
  if (result.status === "error") {
    throw new Error(result.error);
  }
  if (result.data.length === 0) {
    throw new Error("the voice model returned an empty embedding");
  }
  await saveVoiceProfile(humanId, result.data, Math.max(1, spans.length));
}

function saveVoiceProfile(
  humanId: string,
  embedding: number[],
  sampleCount: number,
): Promise<void> {
  return enqueueDatabaseWrite(`voice_profiles:${humanId}`, async () => {
    const now = new Date().toISOString();
    // Biometric data: replaced and forgotten profiles are deleted outright,
    // not tombstoned.
    await executeTransaction([
      {
        sql: `DELETE FROM voice_profiles WHERE human_id = ?`,
        params: [humanId],
      },
      {
        sql: `
          INSERT INTO voice_profiles (
            id, human_id, embedding, dim, model, sample_count,
            created_at, updated_at
          )
          VALUES (?, ?, unhex(?), ?, ?, ?, ?, ?)
        `,
        params: [
          id(),
          humanId,
          encodeEmbeddingHex(embedding),
          embedding.length,
          VOICE_EMBEDDING_MODEL,
          sampleCount,
          now,
          now,
        ],
      },
    ]);
  });
}

export function forgetVoiceProfile(humanId: string): Promise<void> {
  return enqueueDatabaseWrite(`voice_profiles:${humanId}`, async () => {
    await executeTransaction([
      {
        sql: `DELETE FROM voice_profiles WHERE human_id = ?`,
        params: [humanId],
      },
    ]);
  });
}

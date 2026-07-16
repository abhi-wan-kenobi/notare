import { writeText } from "@tauri-apps/plugin-clipboard-manager";

import type { LiveTranscriptSegment } from "@hypr/plugin-transcription";

import { listenerStore } from "~/store/zustand/listener/instance";

export type LiveTranscriptCopyKind = "latest" | "full";

type LiveTranscriptTextSource = {
  liveSegments: LiveTranscriptSegment[];
  liveCaptionText: string;
};

/** In-app hotkeys (react-hotkeys-hook format). */
export const COPY_LATEST_LIVE_CHUNK_HOTKEY = "mod+shift+c";
export const COPY_FULL_LIVE_TRANSCRIPT_HOTKEY = "mod+shift+f";

/** Native-menu accelerator strings for the same mappings. */
export const COPY_LATEST_LIVE_CHUNK_ACCELERATOR = "CmdOrCtrl+Shift+C";
export const COPY_FULL_LIVE_TRANSCRIPT_ACCELERATOR = "CmdOrCtrl+Shift+F";

export function getLiveTranscriptShortcutHints(
  isMacLike: boolean = detectMacLike(),
): { latest: string; full: string } {
  return isMacLike
    ? { latest: "⌘⇧C", full: "⌘⇧F" }
    : { latest: "Ctrl+Shift+C", full: "Ctrl+Shift+F" };
}

function detectMacLike(): boolean {
  if (typeof navigator === "undefined") {
    return false;
  }

  return /mac/i.test(navigator.userAgent);
}

export function getLiveSegmentText(segment: LiveTranscriptSegment): string {
  const wordText = segment.words
    .map((word) => word.text.trim())
    .filter(Boolean)
    .join(" ")
    .replace(/\s+([,.?!;:])/g, "$1");

  return (wordText || segment.text).trim().replace(/\s+/g, " ");
}

/**
 * The most recent utterance: the text of the live segment that started last,
 * falling back to the rolling caption text before the first segment arrives.
 */
export function getLatestLiveTranscriptChunk(
  source: LiveTranscriptTextSource,
): string {
  const latestSegment = source.liveSegments.reduce<LiveTranscriptSegment | null>(
    (latest, segment) =>
      !latest ||
      segment.start_ms > latest.start_ms ||
      (segment.start_ms === latest.start_ms && segment.end_ms >= latest.end_ms)
        ? segment
        : latest,
    null,
  );

  if (latestSegment) {
    const text = getLiveSegmentText(latestSegment);
    if (text) {
      return text;
    }
  }

  return normalizeCaptionText(source.liveCaptionText);
}

/**
 * Everything transcribed so far in the running session. The caption text
 * accumulates all final words plus the current partial tail, so it is the
 * most complete plain-text view; live segments are the fallback.
 */
export function getFullLiveTranscriptText(
  source: LiveTranscriptTextSource,
): string {
  const captionText = normalizeCaptionText(source.liveCaptionText);
  if (captionText) {
    return captionText;
  }

  return source.liveSegments
    .slice()
    .sort((a, b) => a.start_ms - b.start_ms || a.end_ms - b.end_ms)
    .map((segment) => getLiveSegmentText(segment))
    .filter(Boolean)
    .join("\n");
}

function normalizeCaptionText(text: string): string {
  return text.trim().replace(/\s+/g, " ");
}

export function isLiveTranscriptCopyAvailable(): boolean {
  return listenerStore.getState().live.status === "active";
}

/**
 * Copies the requested live-transcript text to the system clipboard.
 * Returns what happened so callers can decide on user feedback:
 * - "copied": text is on the clipboard
 * - "empty": recording is active but there is nothing to copy yet
 * - "inactive": no live session is running (callers should stay silent)
 */
export async function copyLiveTranscript(
  kind: LiveTranscriptCopyKind,
): Promise<"copied" | "empty" | "inactive"> {
  const state = listenerStore.getState();
  if (state.live.status !== "active") {
    return "inactive";
  }

  const text =
    kind === "latest"
      ? getLatestLiveTranscriptChunk(state)
      : getFullLiveTranscriptText(state);

  if (!text) {
    return "empty";
  }

  try {
    await writeText(text);
  } catch {
    // Fall back to the browser clipboard when the plugin is unavailable.
    await navigator.clipboard.writeText(text);
  }

  return "copied";
}

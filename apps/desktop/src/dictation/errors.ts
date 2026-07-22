/**
 * Heuristic for the most common dictation-start failure: engine contention.
 *
 * The dictation orb streams to the internal local-STT (whisper) server. When a
 * batch (re)transcription is already using that engine, the live dictation
 * session can't start and the backend returns a connection/session error. We use
 * this to show a "busy — try again" message instead of dumping the raw backend
 * error string at the user (the raw error is still logged to the console).
 *
 * It is a best-effort classifier over the backend error text, so it errs toward
 * the actionable "busy" message only for signals that plausibly mean contention;
 * anything else falls back to the generic guidance.
 */
export function isLikelyEngineBusyError(rawError?: string): boolean {
  if (!rawError) {
    return false;
  }
  const e = rawError.toLowerCase();
  return (
    e.includes("busy") ||
    e.includes("in use") ||
    e.includes("already") ||
    e.includes("connect") ||
    e.includes("refus") ||
    e.includes("unavailable") ||
    e.includes("session") ||
    e.includes("timed out") ||
    e.includes("timeout") ||
    e.includes("409")
  );
}

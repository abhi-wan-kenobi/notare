/**
 * Cross-mount seek channel (WS-B2 jump-to-source).
 *
 * A search/action-item hit lives in a different tab from the session's audio
 * player, and clicking it opens the session tab where the `AudioPlayerProvider`
 * mounts fresh — there's no direct handle to call `seek()` on. This tiny module
 * bridges that: the producer records a pending seek for a session id; the
 * player consumes it once, when it's ready for that session.
 *
 * Deliberately a module-level store (not React state / context): producer and
 * consumer are in different, non-nested subtrees, and the request must survive
 * the target tab mounting after the click.
 */

type PendingSeek = { sessionId: string; ms: number };

let pending: PendingSeek | null = null;
const listeners = new Set<() => void>();

function notify() {
  for (const l of listeners) l();
}

/**
 * Ask the audio player for `sessionId` to seek to `ms` (transcript
 * jump-to-source). Overwrites any earlier unconsumed request. `ms < 0` clears.
 */
export function requestSeek(sessionId: string, ms: number): void {
  pending = ms >= 0 ? { sessionId, ms } : null;
  notify();
}

/**
 * If a pending seek targets `sessionId`, return its ms and CLEAR it (single
 * consumer, single delivery); otherwise null.
 */
export function consumeSeek(sessionId: string): number | null {
  if (pending && pending.sessionId === sessionId) {
    const ms = pending.ms;
    pending = null;
    notify();
    return ms;
  }
  return null;
}

/** Is there a pending seek for this session (without consuming)? */
export function hasPendingSeek(sessionId: string): boolean {
  return pending?.sessionId === sessionId;
}

/** Subscribe to pending-seek changes (for the player's effect). Returns an unsubscribe. */
export function subscribePendingSeek(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

/** Test-only reset. */
export function __resetPendingSeek(): void {
  pending = null;
  listeners.clear();
}

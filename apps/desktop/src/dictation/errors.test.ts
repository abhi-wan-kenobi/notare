import { describe, expect, test } from "vitest";

import { isLikelyEngineBusyError } from "./errors";

describe("isLikelyEngineBusyError", () => {
  // The point of the fix (PG-10): a dictation-start failure while a batch
  // (re)transcription holds the internal whisper server should read as a
  // "busy — try again" case, so the UI can guide the user instead of dumping
  // the raw backend error.
  test("classifies contention/connection errors as busy", () => {
    for (
      const e of [
        "Session(soniqo_live_start_failed: connection refused)",
        "failed to connect to ws://127.0.0.1:52817/v1/listen",
        "model is already in use",
        "server unavailable",
        "request timed out",
        "HTTP 409 Conflict",
      ]
    ) {
      expect(isLikelyEngineBusyError(e)).toBe(true);
    }
  });

  test("does not over-claim busy for unrelated errors", () => {
    for (
      const e of [undefined, "", "no microphone permission", "invalid model id"]
    ) {
      expect(isLikelyEngineBusyError(e)).toBe(false);
    }
  });
});

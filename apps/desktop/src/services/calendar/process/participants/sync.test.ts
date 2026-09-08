import { describe, expect, test, vi } from "vitest";

import type { ParticipantSyncSnapshot } from "../../storage";
import { syncSessionParticipants } from "./sync";

vi.mock("~/shared/utils", () => ({
  id: () => "human-new",
}));

vi.mock("~/shared/ids", () => ({
  humanIdForEmail: async (email: string) => {
    const normalized = email.trim().toLowerCase();
    return normalized ? `h_${normalized}` : null;
  },
}));

function createSnapshot(
  overrides: Partial<ParticipantSyncSnapshot> = {},
): ParticipantSyncSnapshot {
  return {
    sessions: [],
    humans: [],
    mappings: [],
    ...overrides,
  };
}

const session = {
  id: "session-1",
  ownerUserId: "user-1",
  eventJson: JSON.stringify({ tracking_id: "tracking-1" }),
  trackingId: "tracking-1",
};

describe("syncSessionParticipants", () => {
  test("returns empty output when no events are provided", async () => {
    const result = await syncSessionParticipants({
      incomingParticipants: new Map(),
      snapshot: createSnapshot(),
    });

    expect(result.toAdd).toEqual([]);
    expect(result.toDelete).toEqual([]);
    expect(result.humansToCreate).toEqual([]);
  });

  test("skips events without an associated session", async () => {
    const result = await syncSessionParticipants({
      incomingParticipants: new Map([
        ["tracking-1", [{ email: "test@example.com", name: "Test" }]],
      ]),
      snapshot: createSnapshot(),
    });

    expect(result.toAdd).toEqual([]);
    expect(result.humansToCreate).toEqual([]);
  });

  test("creates a human when the participant email is new", async () => {
    const result = await syncSessionParticipants({
      incomingParticipants: new Map([
        ["tracking-1", [{ email: "new@example.com", name: "New Person" }]],
      ]),
      snapshot: createSnapshot({ sessions: [session] }),
    });

    expect(result.humansToCreate).toEqual([
      {
        id: "h_new@example.com",
        ownerUserId: "user-1",
        email: "new@example.com",
        name: "New Person",
      },
    ]);
    expect(result.toAdd).toEqual([
      {
        sessionId: "session-1",
        humanId: "h_new@example.com",
        email: "new@example.com",
      },
    ]);
  });

  test("uses an existing human when email matches case-insensitively", async () => {
    const result = await syncSessionParticipants({
      incomingParticipants: new Map([
        ["tracking-1", [{ email: "Existing@Example.com", name: "Existing" }]],
      ]),
      snapshot: createSnapshot({
        sessions: [session],
        humans: [{ id: "human-1", email: "existing@example.com" }],
      }),
    });

    expect(result.humansToCreate).toEqual([]);
    expect(result.toAdd[0]).toMatchObject({ humanId: "human-1" });
  });

  test("deletes auto mappings when a participant is removed", async () => {
    const result = await syncSessionParticipants({
      incomingParticipants: new Map([["tracking-1", []]]),
      snapshot: createSnapshot({
        sessions: [session],
        humans: [{ id: "human-1", email: "removed@example.com" }],
        mappings: [
          {
            id: "mapping-1",
            sessionId: "session-1",
            humanId: "human-1",
            source: "auto",
          },
        ],
      }),
    });

    expect(result.toDelete).toEqual(["mapping-1"]);
  });

  test("preserves excluded mappings", async () => {
    const result = await syncSessionParticipants({
      incomingParticipants: new Map([["tracking-1", []]]),
      snapshot: createSnapshot({
        sessions: [session],
        humans: [{ id: "human-1", email: "excluded@example.com" }],
        mappings: [
          {
            id: "mapping-1",
            sessionId: "session-1",
            humanId: "human-1",
            source: "excluded",
          },
        ],
      }),
    });

    expect(result.toDelete).toEqual([]);
  });
});

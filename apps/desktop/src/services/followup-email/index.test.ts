import { renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  liveQueryExecute: vi.fn(),
  loadNoteFile: vi.fn(),
  deliverFollowupEmail: vi.fn(),
  generateText: vi.fn(),
  sonnerToastError: vi.fn(),
  userEmail: "organizer@internal.com",
}));

vi.mock("~/auth", () => ({
  useAuth: () => ({
    session: {
      user: {
        email: mocks.userEmail,
      },
    },
  }),
}));

vi.mock("~/ai/hooks/useLLMConnection", () => ({
  useScopedLanguageModel: () => ({}),
}));

vi.mock("~/db", () => ({
  liveQueryClient: {
    execute: (...args: unknown[]) => mocks.liveQueryExecute(...args),
  },
}));

vi.mock("~/chat/tools/note-files", () => ({
  loadNoteFile: (...args: unknown[]) => mocks.loadNoteFile(...args),
}));

vi.mock("./delivery", () => ({
  deliverFollowupEmail: (...args: unknown[]) =>
    mocks.deliverFollowupEmail(...args),
}));

vi.mock("ai", () => ({
  generateText: (...args: unknown[]) => mocks.generateText(...args),
}));

vi.mock("@hypr/ui/components/ui/toast", () => ({
  sonnerToast: {
    error: (...args: unknown[]) => mocks.sonnerToastError(...args),
    info: vi.fn(),
    success: vi.fn(),
  },
}));

import { useFollowupEmail } from "./index";

describe("useFollowupEmail hook", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.userEmail = "organizer@internal.com";
  });

  it("filters out internal attendees using current user email domain and includes transcript", async () => {
    mocks.liveQueryExecute.mockResolvedValueOnce([
      {
        title: "Client Sync",
        content: "Action item for client.",
        event_json: JSON.stringify({
          attendees: [
            { email: "organizer@internal.com" },
            { email: "colleague@internal.com" },
            { email: "client@external.com" },
          ],
        }),
      },
    ]);

    mocks.loadNoteFile.mockResolvedValueOnce({
      sessionId: "sess-1",
      title: "Client Sync",
      sections: [
        {
          title: "Transcript",
          text: "Client: We agree on the timeline.",
        },
      ],
    });

    mocks.generateText.mockResolvedValueOnce({
      text: "SUBJECT: Follow-up: Client Sync\nBODY:\nThanks for meeting today.",
    });

    mocks.deliverFollowupEmail.mockResolvedValueOnce({
      method: "gmail",
      delivered: "gmail",
      draftId: "draft-999",
    });

    const { result } = renderHook(() => useFollowupEmail("sess-1"));
    const delivery = await result.current.generateAndDeliver();

    expect(delivery).toEqual({
      method: "gmail",
      delivered: "gmail",
      draftId: "draft-999",
    });

    // Verify delivery was called with ONLY external recipients
    expect(mocks.deliverFollowupEmail).toHaveBeenCalledWith({
      to: ["client@external.com"],
      subject: "Follow-up: Client Sync",
      body: "Thanks for meeting today.",
    });

    // Verify LLM prompt included transcript excerpt
    expect(mocks.generateText).toHaveBeenCalledWith(
      expect.objectContaining({
        prompt: expect.stringContaining("Client: We agree on the timeline."),
      }),
    );
  });

  it("surfaces Gmail draft failure via toast and returns delivered: mailto with reason", async () => {
    mocks.liveQueryExecute.mockResolvedValueOnce([
      {
        title: "Client Sync",
        content: "Notes",
        event_json: JSON.stringify({
          attendees: [{ email: "client@external.com" }],
        }),
      },
    ]);

    mocks.loadNoteFile.mockResolvedValueOnce(null);

    mocks.generateText.mockResolvedValueOnce({
      text: "SUBJECT: Follow-up: Client Sync\nBODY:\nHere are the notes.",
    });

    mocks.deliverFollowupEmail.mockResolvedValueOnce({
      method: "mailto",
      delivered: "mailto",
      mailtoUrl: "mailto:client@external.com",
      reason: "Token expired",
    });

    const { result } = renderHook(() => useFollowupEmail("sess-1"));
    const delivery = await result.current.generateAndDeliver();

    expect(delivery).toMatchObject({
      method: "mailto",
      delivered: "mailto",
      reason: "Token expired",
    });

    expect(mocks.sonnerToastError).toHaveBeenCalledWith(
      expect.stringContaining("Token expired"),
    );
  });

  it("toasts and returns null when no external recipients are found", async () => {
    mocks.liveQueryExecute.mockResolvedValueOnce([
      {
        title: "Internal Standup",
        content: "Internal notes only",
        event_json: JSON.stringify({
          attendees: [
            { email: "organizer@internal.com" },
            { email: "colleague@internal.com" },
          ],
        }),
      },
    ]);

    const { result } = renderHook(() => useFollowupEmail("sess-internal"));
    const delivery = await result.current.generateAndDeliver();

    expect(delivery).toBeNull();
    expect(mocks.sonnerToastError).toHaveBeenCalledWith(
      "No external recipients found for this meeting.",
    );
    await vi.waitFor(() => {
      expect(result.current.error).toBe(
        "No external recipients found for this meeting.",
      );
    });
    expect(mocks.deliverFollowupEmail).not.toHaveBeenCalled();
  });
});

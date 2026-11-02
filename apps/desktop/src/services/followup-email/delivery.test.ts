import { beforeEach, describe, expect, it, vi } from "vitest";

const calendarCommands = vi.hoisted(() => ({
  gmailStatus: vi.fn(),
  gmailCreateDraft: vi.fn(),
}));

const openerCommands = vi.hoisted(() => ({
  openUrl: vi.fn(),
}));

vi.mock("@hypr/plugin-calendar", () => ({
  commands: calendarCommands,
}));

vi.mock("@hypr/plugin-opener2", () => ({
  commands: openerCommands,
}));

import { deliverFollowupEmail } from "./delivery";

describe("deliverFollowupEmail", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("creates a Gmail draft when Gmail is connected", async () => {
    calendarCommands.gmailStatus.mockResolvedValue({
      status: "ok",
      data: {
        has_client: true,
        connected: true,
        accounts: ["alice@gmail.com"],
      },
    });

    calendarCommands.gmailCreateDraft.mockResolvedValue({
      status: "ok",
      data: "draft_12345",
    });

    const result = await deliverFollowupEmail({
      to: ["client@example.com"],
      subject: "Follow-up",
      body: "Here is the recap.",
    });

    expect(result.method).toBe("gmail");
    expect(result.draftId).toBe("draft_12345");
    expect(calendarCommands.gmailCreateDraft).toHaveBeenCalledWith(
      null,
      ["client@example.com"],
      "Follow-up",
      "Here is the recap.",
    );
    expect(openerCommands.openUrl).not.toHaveBeenCalled();
  });

  it("falls back to mailto and clipboard when Gmail is not connected", async () => {
    calendarCommands.gmailStatus.mockResolvedValue({
      status: "ok",
      data: {
        has_client: false,
        connected: false,
        accounts: [],
      },
    });

    const clipboardMock = vi.fn().mockResolvedValue(undefined);
    vi.stubGlobal("navigator", {
      clipboard: {
        writeText: clipboardMock,
      },
    });

    const result = await deliverFollowupEmail({
      to: ["client@example.com"],
      subject: "Follow-up",
      body: "Here is the recap.",
    });

    expect(result.method).toBe("mailto");
    expect(result.mailtoUrl).toContain("mailto:client@example.com?");
    expect(clipboardMock).toHaveBeenCalledWith("Here is the recap.");
    expect(openerCommands.openUrl).toHaveBeenCalledWith(result.mailtoUrl, null);
  });
});

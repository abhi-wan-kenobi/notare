import { describe, expect, it, vi } from "vitest";

const calendarCommands = vi.hoisted(() => ({
  listConnectionIds: vi.fn(),
  listCalendars: vi.fn(),
  createEvent: vi.fn(),
}));

vi.mock("@hypr/plugin-calendar", () => ({
  commands: calendarCommands,
}));

import {
  accountEmailFromConnectionId,
  fetchWritableCalendarTargets,
  submitFollowupEvent,
} from "./schedule-followup-queries";

describe("accountEmailFromConnectionId", () => {
  it("strips the provider prefix", () => {
    expect(accountEmailFromConnectionId("google:alice@example.com")).toBe(
      "alice@example.com",
    );
    expect(accountEmailFromConnectionId("outlook:bob@contoso.com")).toBe(
      "bob@contoso.com",
    );
  });

  it("falls back to the raw id when there is no prefix", () => {
    expect(accountEmailFromConnectionId("outlook-direct")).toBe(
      "outlook-direct",
    );
  });
});

describe("fetchWritableCalendarTargets", () => {
  it("keeps only writable google/outlook calendars, skipping apple/ics", async () => {
    calendarCommands.listConnectionIds.mockResolvedValue({
      status: "ok",
      data: [
        { provider: "google", connection_ids: ["google:alice@example.com"] },
        { provider: "outlook", connection_ids: ["outlook:bob@contoso.com"] },
        { provider: "apple", connection_ids: ["apple-direct"] },
        { provider: "ics", connection_ids: ["ics-file-1"] },
      ],
    });
    calendarCommands.listCalendars.mockImplementation(
      async (provider: string, connectionId: string) => {
        if (provider === "google") {
          return {
            status: "ok",
            data: [
              { id: "primary", title: "Alice", can_edit: true },
              { id: "shared", title: "Shared (view only)", can_edit: false },
              { id: "unknown", title: "Unknown access", can_edit: null },
            ],
          };
        }
        if (provider === "outlook") {
          return {
            status: "ok",
            data: [{ id: "cal-1", title: "Bob's calendar", can_edit: true }],
          };
        }
        throw new Error(`unexpected provider ${provider} (${connectionId})`);
      },
    );

    const targets = await fetchWritableCalendarTargets();

    expect(targets).toEqual([
      {
        key: "google::google:alice@example.com::primary",
        provider: "google",
        connectionId: "google:alice@example.com",
        calendarTrackingId: "primary",
        calendarTitle: "Alice",
        accountEmail: "alice@example.com",
      },
      {
        key: "outlook::outlook:bob@contoso.com::cal-1",
        provider: "outlook",
        connectionId: "outlook:bob@contoso.com",
        calendarTrackingId: "cal-1",
        calendarTitle: "Bob's calendar",
        accountEmail: "bob@contoso.com",
      },
    ]);
    expect(calendarCommands.listCalendars).not.toHaveBeenCalledWith(
      "apple",
      expect.anything(),
    );
    expect(calendarCommands.listCalendars).not.toHaveBeenCalledWith(
      "ics",
      expect.anything(),
    );
  });

  it("returns an empty list when there are no connections", async () => {
    calendarCommands.listConnectionIds.mockResolvedValue({
      status: "ok",
      data: [],
    });
    expect(await fetchWritableCalendarTargets()).toEqual([]);
  });

  it("returns an empty list when listing connections errors", async () => {
    calendarCommands.listConnectionIds.mockResolvedValue({
      status: "error",
      error: "not connected",
    });
    expect(await fetchWritableCalendarTargets()).toEqual([]);
  });

  it("skips a connection whose calendar listing errors, keeping the rest", async () => {
    calendarCommands.listConnectionIds.mockResolvedValue({
      status: "ok",
      data: [
        {
          provider: "google",
          connection_ids: [
            "google:broken@example.com",
            "google:ok@example.com",
          ],
        },
      ],
    });
    calendarCommands.listCalendars.mockImplementation(
      async (_provider: string, connectionId: string) => {
        if (connectionId === "google:broken@example.com") {
          return { status: "error", error: "token expired" };
        }
        return {
          status: "ok",
          data: [{ id: "primary", title: "OK", can_edit: true }],
        };
      },
    );

    const targets = await fetchWritableCalendarTargets();
    expect(targets).toHaveLength(1);
    expect(targets[0]!.connectionId).toBe("google:ok@example.com");
  });
});

describe("submitFollowupEvent", () => {
  const target = {
    key: "google::google:alice@example.com::primary",
    provider: "google" as const,
    connectionId: "google:alice@example.com",
    calendarTrackingId: "primary",
    calendarTitle: "Alice",
    accountEmail: "alice@example.com",
  };

  it("calls createEvent with the chosen calendar and mapped attendees", async () => {
    calendarCommands.createEvent.mockResolvedValue({
      status: "ok",
      data: "event-123",
    });

    const eventId = await submitFollowupEvent({
      target,
      title: "Weekly sync — follow-up",
      startIso: "2026-11-09T14:30:00.000Z",
      endIso: "2026-11-09T15:00:00.000Z",
      description: "- Send proposal",
      attendees: [{ id: "h1", name: "Bob", email: "bob@example.com" }],
    });

    expect(eventId).toBe("event-123");
    expect(calendarCommands.createEvent).toHaveBeenCalledWith(
      "google",
      "google:alice@example.com",
      {
        calendar_tracking_id: "primary",
        title: "Weekly sync — follow-up",
        started_at: "2026-11-09T14:30:00.000Z",
        ended_at: "2026-11-09T15:00:00.000Z",
        is_all_day: false,
        location: null,
        notes: "- Send proposal",
        url: null,
        attendees: [{ email: "bob@example.com", name: "Bob" }],
      },
    );
  });

  it("sends null attendees and notes when there are none", async () => {
    calendarCommands.createEvent.mockResolvedValue({
      status: "ok",
      data: "event-456",
    });

    await submitFollowupEvent({
      target,
      title: "Follow-up",
      startIso: "2026-11-09T14:30:00.000Z",
      endIso: "2026-11-09T15:00:00.000Z",
      description: "   ",
      attendees: [],
    });

    expect(calendarCommands.createEvent).toHaveBeenCalledWith(
      "google",
      "google:alice@example.com",
      expect.objectContaining({ attendees: null, notes: null }),
    );
  });

  it("throws with the backend error message on failure", async () => {
    calendarCommands.createEvent.mockResolvedValue({
      status: "error",
      error: "consent denied",
    });

    await expect(
      submitFollowupEvent({
        target,
        title: "Follow-up",
        startIso: "2026-11-09T14:30:00.000Z",
        endIso: "2026-11-09T15:00:00.000Z",
        description: "",
        attendees: [],
      }),
    ).rejects.toThrow("consent denied");
  });
});

import { describe, expect, it } from "vitest";

import {
  addMinutesIso,
  datetimeLocalToIso,
  defaultFollowupAttendees,
  defaultFollowupDescription,
  defaultFollowupStart,
  defaultFollowupTitle,
  nextBusinessDay,
  toDatetimeLocalValue,
} from "./schedule-followup-defaults";

describe("nextBusinessDay", () => {
  it("rolls Friday forward to Monday", () => {
    // Friday 2026-11-06, 14:30 local
    const friday = new Date(2026, 10, 6, 14, 30);
    const next = nextBusinessDay(friday);
    expect(next.getDay()).toBe(1); // Monday
    expect(next.getDate()).toBe(9);
    expect(next.getHours()).toBe(14);
    expect(next.getMinutes()).toBe(30);
  });

  it("rolls Saturday forward to Monday", () => {
    const saturday = new Date(2026, 10, 7, 9, 0);
    const next = nextBusinessDay(saturday);
    expect(next.getDay()).toBe(1);
    expect(next.getDate()).toBe(9);
  });

  it("rolls Sunday forward to Monday", () => {
    const sunday = new Date(2026, 10, 8, 9, 0);
    const next = nextBusinessDay(sunday);
    expect(next.getDay()).toBe(1);
    expect(next.getDate()).toBe(9);
  });

  it("advances a weekday by exactly one day", () => {
    const tuesday = new Date(2026, 10, 3, 11, 0);
    const next = nextBusinessDay(tuesday);
    expect(next.getDay()).toBe(3); // Wednesday
    expect(next.getDate()).toBe(4);
  });
});

describe("defaultFollowupStart", () => {
  it("defaults to the next business day at the same time", () => {
    const now = new Date(2026, 10, 6, 14, 30); // Friday
    const start = defaultFollowupStart(now);
    expect(start.getDay()).toBe(1);
    expect(start.getHours()).toBe(14);
    expect(start.getMinutes()).toBe(30);
  });
});

describe("defaultFollowupTitle", () => {
  it("appends the follow-up suffix to the meeting title", () => {
    expect(defaultFollowupTitle("Weekly sync")).toBe("Weekly sync — follow-up");
  });

  it("falls back to a generic title when none is given", () => {
    expect(defaultFollowupTitle(null)).toBe("Meeting — follow-up");
    expect(defaultFollowupTitle("   ")).toBe("Meeting — follow-up");
  });
});

describe("local datetime <-> ISO conversion (timezone)", () => {
  it("round-trips a local wall-clock value through ISO regardless of host timezone", () => {
    const original = new Date(2026, 10, 9, 9, 15); // local wall clock
    const localValue = toDatetimeLocalValue(original);
    const iso = datetimeLocalToIso(localValue);
    expect(iso).toBe(original.toISOString());
  });

  it("returns null for an empty or invalid value", () => {
    expect(datetimeLocalToIso("")).toBeNull();
    expect(datetimeLocalToIso("not-a-date")).toBeNull();
  });
});

describe("addMinutesIso", () => {
  it("adds the duration to produce the end timestamp", () => {
    const start = new Date(2026, 10, 9, 9, 0).toISOString();
    const end = addMinutesIso(start, 30);
    expect(end).toBe(
      new Date(new Date(start).getTime() + 30 * 60_000).toISOString(),
    );
  });
});

describe("defaultFollowupDescription", () => {
  it("bullets open action items and skips done ones", () => {
    const description = defaultFollowupDescription([
      { text: "Send proposal", status: "todo" },
      { text: "Book venue", status: "done" },
      { text: "Confirm budget", status: "in_progress" },
      { text: "  ", status: "todo" },
    ]);
    expect(description).toBe("- Send proposal\n- Confirm budget");
  });

  it("returns an empty string when there are no open items", () => {
    expect(
      defaultFollowupDescription([{ text: "Book venue", status: "done" }]),
    ).toBe("");
    expect(defaultFollowupDescription([])).toBe("");
  });
});

describe("defaultFollowupAttendees", () => {
  it("includes only participants with a valid email, deduped", () => {
    const attendees = defaultFollowupAttendees([
      { humanId: "h1", name: "Alice", email: "alice@example.com" },
      { humanId: "h2", name: "No Email", email: "" },
      { humanId: "h3", name: "Dup", email: "Alice@Example.com" },
      { humanId: "h4", name: "Bob", email: "bob@example.com" },
    ]);
    expect(attendees).toEqual([
      { id: "h1", name: "Alice", email: "alice@example.com" },
      { id: "h4", name: "Bob", email: "bob@example.com" },
    ]);
  });
});

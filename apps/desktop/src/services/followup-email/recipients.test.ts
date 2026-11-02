import { describe, expect, it } from "vitest";

import { extractExternalAttendees, getEmailDomain } from "./recipients";

describe("extractExternalAttendees", () => {
  it("extracts email domain properly", () => {
    expect(getEmailDomain("alice@example.com")).toBe("example.com");
    expect(getEmailDomain("bob@sub.domain.co.uk")).toBe("sub.domain.co.uk");
    expect(getEmailDomain("invalid")).toBeNull();
  });

  it("filters out the current user by isCurrentUser flag or email", () => {
    const attendees = [
      { email: "me@company.com", isCurrentUser: true },
      { email: "other-me@company.com" },
      { email: "client@external.com" },
    ];

    const result = extractExternalAttendees(attendees, {
      currentUserEmail: "other-me@company.com",
    });

    expect(result).toEqual(["client@external.com"]);
  });

  it("filters out attendees with matching internal domain", () => {
    const attendees = [
      { email: "alice@acme.com" },
      { email: "bob@acme.com" },
      { email: "carol@partner.com" },
      { email: "dan@vendor.org" },
    ];

    const result = extractExternalAttendees(attendees, {
      currentUserEmail: "alice@acme.com",
    });

    expect(result).toEqual(["carol@partner.com", "dan@vendor.org"]);
  });

  it("caps external recipients at 10", () => {
    const attendees = Array.from({ length: 15 }, (_, i) => ({
      email: `client${i + 1}@external.com`,
    }));

    const result = extractExternalAttendees(attendees, {
      currentUserEmail: "organizer@internal.com",
    });

    expect(result).toHaveLength(10);
    expect(result[0]).toBe("client1@external.com");
    expect(result[9]).toBe("client10@external.com");
  });

  it("deduplicates case-insensitively and skips malformed emails", () => {
    const attendees = [
      "PARTNER@external.com",
      "partner@external.com",
      "not-an-email",
      "",
      "  second@external.com  ",
    ];

    const result = extractExternalAttendees(attendees, {
      internalDomain: "internal.com",
    });

    expect(result).toEqual(["partner@external.com", "second@external.com"]);
  });

  it("does not exclude public email providers like gmail as internal domain unless specified", () => {
    const attendees = [
      { email: "friend1@gmail.com" },
      { email: "friend2@gmail.com" },
    ];

    const result = extractExternalAttendees(attendees, {
      currentUserEmail: "me@gmail.com",
    });

    // Both friends are external (gmail is not treated as a private org domain)
    expect(result).toEqual(["friend1@gmail.com", "friend2@gmail.com"]);
  });
});

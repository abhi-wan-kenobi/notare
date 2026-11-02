import { describe, expect, it } from "vitest";

import { buildMailtoUrl } from "./mailto";

describe("buildMailtoUrl", () => {
  it("encodes subject and body correctly with special characters and newlines", () => {
    const to = ["alice@example.com", "bob@example.com"];
    const subject = "Follow-up: Q4 Roadmap & Sync (10/15)";
    const body =
      "Hi Alice & Bob,\n\nHere are the action items:\n- Review PR #42\n\nThanks!";

    const url = buildMailtoUrl(to, subject, body);

    expect(url.startsWith("mailto:alice@example.com,bob@example.com?")).toBe(
      true,
    );
    expect(url).toContain(`subject=${encodeURIComponent(subject)}`);
    expect(url).toContain(`body=${encodeURIComponent(body)}`);

    // Verify round-trip parsing
    const urlObj = new URL(url);
    expect(urlObj.pathname).toBe("alice@example.com,bob@example.com");
    expect(urlObj.searchParams.get("subject")).toBe(subject);
    expect(urlObj.searchParams.get("body")).toBe(body);
  });

  it("handles empty recipients", () => {
    const url = buildMailtoUrl([], "Test Subject", "Test Body");
    expect(url.startsWith("mailto:?")).toBe(true);
    expect(url).toContain("subject=Test%20Subject");
    expect(url).toContain("body=Test%20Body");
  });

  it("handles empty subject or body", () => {
    expect(buildMailtoUrl(["alice@example.com"], "", "")).toBe(
      "mailto:alice@example.com",
    );
    expect(buildMailtoUrl(["alice@example.com"], "Hi", "")).toBe(
      "mailto:alice@example.com?subject=Hi",
    );
  });
});

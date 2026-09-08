import { describe, expect, it } from "vitest";

import { humanIdForEmail, organizationIdForName, participantId } from "./ids";

// The shared test vector: byte-identical with the Rust unit test in
// crates/db-app/src/id_backfill.rs, so the TS and Rust minting paths are
// provably the same function.
describe("deterministic ids", () => {
  it("matches the shared cross-implementation test vector", async () => {
    await expect(humanIdForEmail("alice@Example.com ")).resolves.toBe(
      "h_4fa1d59c20dd48d285b666b890067c7f",
    );
    await expect(organizationIdForName("  Example Inc  ")).resolves.toBe(
      "o_b43677833502048ea625456d505a4c33",
    );

    const humanId = await humanIdForEmail("alice@example.com");
    expect(humanId).toBe("h_4fa1d59c20dd48d285b666b890067c7f");
    await expect(participantId("session-1", humanId!, "")).resolves.toBe(
      "sp_668289d5958cac9b021d2b3637a4b4f7",
    );
    await expect(
      participantId("session-1", "", "Bob@Example.com"),
    ).resolves.toBe("sp_21ea03a4de5b32b54dce588d3e6441ea");
  });

  it("normalizes case and surrounding whitespace before hashing", async () => {
    await expect(humanIdForEmail("alice@example.com")).resolves.toBe(
      await humanIdForEmail("  ALICE@example.com "),
    );
  });

  it("normalizes Unicode form before hashing, so composed and decomposed accents converge", async () => {
    // Precomposed "e-acute" (U+00E9) vs. "e" + combining acute accent
    // (U+0065 U+0301): same visible string, different bytes.
    const composed = "caf\u00e9@example.com";
    const decomposed = "cafe\u0301@example.com";
    expect(composed).not.toBe(decomposed);
    await expect(humanIdForEmail(composed)).resolves.toBe(
      await humanIdForEmail(decomposed),
    );
  });

  it("keeps a random UUID when the identity input is empty", async () => {
    await expect(humanIdForEmail("")).resolves.toBeNull();
    await expect(humanIdForEmail("   ")).resolves.toBeNull();
    await expect(organizationIdForName("")).resolves.toBeNull();
    await expect(participantId("session-1", "", "")).resolves.toBeNull();
  });

  it("prefers the human key over the email key for participants", async () => {
    const byHuman = await participantId("s", "h_abc", "ignored@x.com");
    const byHumanNoEmail = await participantId("s", "h_abc", "");
    expect(byHuman).toBe(byHumanNoEmail);
    expect(byHuman).toMatch(/^sp_[0-9a-f]{32}$/);
  });

  it("separates participants by session", async () => {
    const a = await participantId("session-a", "", "bob@example.com");
    const b = await participantId("session-b", "", "bob@example.com");
    expect(a).not.toBe(b);
  });
});

import { describe, expect, it } from "vitest";

import { assembleFollowupPrompt, parseFollowupOutput } from "./draft";

describe("draft assembly & parsing", () => {
  it("assembles prompt with meeting context, attendees, notes, and transcript", () => {
    const prompt = assembleFollowupPrompt({
      sessionTitle: "Sprint 42 Planning",
      attendees: ["alice@example.com", "bob@example.com"],
      notesContent:
        "- Completed auth module\n- Decided to launch beta next week",
      transcriptText: "Alice: I will handle the docs by Friday.",
    });

    expect(prompt).toContain("Meeting: Sprint 42 Planning");
    expect(prompt).toContain("Attendees: alice@example.com, bob@example.com");
    expect(prompt).toContain("--- Meeting Notes ---\n- Completed auth module");
    expect(prompt).toContain(
      "--- Transcript Excerpt ---\nAlice: I will handle the docs by Friday.",
    );
  });

  it("parses subject and body correctly when format is followed", () => {
    const output = `
SUBJECT: Follow-up: Sprint 42 Planning & Roadmap
BODY:
Hi team,

Here is a quick recap of our discussion:
- Reviewed Sprint 41 velocity.

Decisions:
- Launch beta on Monday.

Action Items:
- Alice: Update documentation (Due Friday)
- Bob: Finalize integration tests (Due Thursday)

Next Steps:
- Check-in on Monday 10am.
`;

    const draft = parseFollowupOutput(output);
    expect(draft.subject).toBe("Follow-up: Sprint 42 Planning & Roadmap");
    expect(draft.body).toContain("Here is a quick recap of our discussion:");
    expect(draft.body).toContain(
      "Action Items:\n- Alice: Update documentation",
    );
  });

  it("falls back gracefully when output lacks strict markers", () => {
    const output =
      "Team, here are the meeting notes and action items from today.";
    const draft = parseFollowupOutput(output, "Default Subject");

    expect(draft.subject).toBe("Default Subject");
    expect(draft.body).toBe(output);
  });
});

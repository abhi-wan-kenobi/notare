import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import type { LanguageModel } from "ai";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { BriefCard } from "./brief-card";
import type { SessionBrief } from "./types";

const mocks = vi.hoisted(() => ({
  briefForEvent: null as SessionBrief | null,
  briefForSession: null as SessionBrief | null,
  dismissedId: null as string | null,
  dismissShouldFail: false,
  regeneratedEventId: null as string | null,
  taskModel: {
    model: {} as unknown as LanguageModel,
    resolution: {
      status: "ok" as const,
      providerId: "ollama",
      modelId: "llama3",
    },
    target: null,
  },
}));

vi.mock("./storage", () => ({
  useBriefForEvent: () => mocks.briefForEvent,
  useBriefForSession: () => mocks.briefForSession,
  dismissBrief: vi.fn(async (id: string) => {
    if (mocks.dismissShouldFail) {
      throw new Error("DB offline");
    }
    mocks.dismissedId = id;
  }),
}));

vi.mock("./generator", () => ({
  regenerateBriefForEvent: vi.fn(async (eventId: string) => {
    mocks.regeneratedEventId = eventId;
    return null;
  }),
}));

vi.mock("~/services/llm-router", () => ({
  useTaskModel: () => mocks.taskModel,
}));

vi.mock("~/chat/citations/chip", () => ({
  CitationChip: ({ sessionId, ms }: { sessionId: string; ms: number }) => (
    <span data-testid="citation-chip" data-session-id={sessionId} data-ms={ms}>
      [Source: {sessionId}@{ms}]
    </span>
  ),
}));

describe("BriefCard Component", () => {
  const sampleBrief: SessionBrief = {
    id: "brief-test-1",
    session_id: "sess-1",
    event_id: "event-1",
    generated_at: "2026-11-02T10:00:00.000Z",
    content_md:
      "## Attendees\n- Alice (Acme)\n\n## Recent Meetings\n- Discussed renewal [[src:sess-prev@1000]]\n\n## Action Items\n- Send report [[src:sess-prev@0]]",
    model: "ollama/llama3",
    dismissed_at: null,
    created_at: "2026-11-02T10:00:00.000Z",
    updated_at: "2026-11-02T10:00:00.000Z",
  };

  beforeEach(() => {
    vi.clearAllMocks();
    mocks.briefForEvent = null;
    mocks.briefForSession = null;
    mocks.dismissedId = null;
    mocks.dismissShouldFail = false;
    mocks.regeneratedEventId = null;
  });

  afterEach(() => {
    cleanup();
  });

  it("renders null when no brief is found", () => {
    const { container } = render(<BriefCard eventId="no-brief-event" />);
    expect(container.firstChild).toBeNull();
  });

  it("renders the brief and collapsible header when brief is provided", () => {
    render(<BriefCard initialBrief={sampleBrief} />);

    expect(screen.getByText("Brief")).toBeDefined();
    expect(screen.getByText("Attendees")).toBeDefined();
    expect(screen.getByText("Alice (Acme)")).toBeDefined();

    // Verify citations are rendered through CitationChip
    const citationChips = screen.getAllByTestId("citation-chip");
    expect(citationChips).toHaveLength(2);
    expect(citationChips[0]?.getAttribute("data-session-id")).toBe("sess-prev");
    expect(citationChips[0]?.getAttribute("data-ms")).toBe("1000");
  });

  it("toggles collapse and expand when header is clicked", () => {
    const { container } = render(<BriefCard initialBrief={sampleBrief} />);

    const toggleButton = container.querySelector(
      "[data-brief-toggle]",
    ) as HTMLElement;
    expect(toggleButton.getAttribute("aria-expanded")).toBe("true");
    expect(screen.getByText("Attendees")).toBeDefined();

    // Click to collapse
    fireEvent.click(toggleButton);
    expect(toggleButton.getAttribute("aria-expanded")).toBe("false");
    expect(screen.queryByText("Attendees")).toBeNull();

    // Click to re-expand
    fireEvent.click(toggleButton);
    expect(toggleButton.getAttribute("aria-expanded")).toBe("true");
    expect(screen.getByText("Attendees")).toBeDefined();
  });

  it("calls regenerate when regenerate button is clicked", async () => {
    render(<BriefCard initialBrief={sampleBrief} />);

    const regenerateBtn = screen.getByTitle("Regenerate brief");
    fireEvent.click(regenerateBtn);

    expect(mocks.regeneratedEventId).toBe("event-1");
  });

  it("calls dismiss and hides the card when dismiss button is clicked", async () => {
    render(<BriefCard initialBrief={sampleBrief} />);

    const dismissBtn = screen.getByTitle("Dismiss brief");
    fireEvent.click(dismissBtn);

    expect(mocks.dismissedId).toBe("brief-test-1");
    // Card should unmount / disappear
    expect(screen.queryByText("Brief")).toBeNull();
  });

  it("reverts optimistic dismiss when dismissBrief fails", async () => {
    mocks.dismissShouldFail = true;
    render(<BriefCard initialBrief={sampleBrief} />);

    const dismissBtn = screen.getByTitle("Dismiss brief");
    fireEvent.click(dismissBtn);

    // Card should still be visible because dismiss failed and reverted
    await vi.waitFor(() => {
      expect(screen.getByText("Brief")).toBeDefined();
    });
  });

  it("renders citation chips inside headings", () => {
    const headingBrief: SessionBrief = {
      ...sampleBrief,
      content_md: "## Meeting Outcomes [[src:sess-prev@2500]]\n- Followed up",
    };
    render(<BriefCard initialBrief={headingBrief} />);

    const chips = screen.getAllByTestId("citation-chip");
    expect(chips).toHaveLength(1);
    expect(chips[0]?.getAttribute("data-session-id")).toBe("sess-prev");
    expect(chips[0]?.getAttribute("data-ms")).toBe("2500");
  });

  it("ignores initialBrief if it has already been dismissed", () => {
    const dismissedBrief: SessionBrief = {
      ...sampleBrief,
      dismissed_at: "2026-11-02T12:00:00.000Z",
    };
    const { container } = render(<BriefCard initialBrief={dismissedBrief} />);
    expect(container.firstChild).toBeNull();
  });
});

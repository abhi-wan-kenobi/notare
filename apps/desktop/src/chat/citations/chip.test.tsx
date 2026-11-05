import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { CitationChip } from "./chip";

import * as pendingSeek from "~/audio-player/pending-seek";
import { useSessionSummaries } from "~/session/queries";
import { useTabs } from "~/store/zustand/tabs";

vi.mock("~/session/queries", () => ({
  useSessionSummaries: vi.fn(),
}));

vi.mock("~/audio-player/pending-seek", () => ({
  requestSeek: vi.fn(),
}));

describe("CitationChip", () => {
  afterEach(() => {
    cleanup();
  });

  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("renders as plain text when sessionId is unknown", () => {
    vi.mocked(useSessionSummaries).mockReturnValue([]);

    render(<CitationChip sessionId="missing-session" ms={5000} />);

    expect(screen.queryByRole("button")).toBeNull();
    expect(screen.getByText("[[src:missing-session@5000]]")).toBeDefined();
  });

  it("renders a chip with title and timestamp when session is found", () => {
    vi.mocked(useSessionSummaries).mockReturnValue([
      {
        id: "session-1",
        title: "Product Sync",
        created_at: "2026-11-01T10:00:00Z",
      },
    ]);

    render(<CitationChip sessionId="session-1" ms={75000} />);

    const button = screen.getByRole("button");
    expect(button).toBeDefined();
    expect(screen.getByText("Product Sync")).toBeDefined();
    expect(screen.getByText("01:15")).toBeDefined();
  });

  it("navigates and requests seek when clicked", () => {
    vi.mocked(useSessionSummaries).mockReturnValue([
      {
        id: "session-1",
        title: "Architecture Review",
        created_at: "2026-11-01T10:00:00Z",
      },
    ]);

    const openNewSpy = vi.fn();
    useTabs.setState({ openNew: openNewSpy });

    render(<CitationChip sessionId="session-1" ms={120000} />);

    const button = screen.getByRole("button");
    fireEvent.click(button);

    expect(pendingSeek.requestSeek).toHaveBeenCalledWith("session-1", 120000);
    expect(openNewSpy).toHaveBeenCalledWith({
      type: "sessions",
      id: "session-1",
    });
  });

  it("calls onNavigate callback when provided", () => {
    vi.mocked(useSessionSummaries).mockReturnValue([
      {
        id: "session-1",
        title: "Team Retrospective",
        created_at: "2026-11-01T10:00:00Z",
      },
    ]);

    const onNavigate = vi.fn();
    render(
      <CitationChip sessionId="session-1" ms={30000} onNavigate={onNavigate} />,
    );

    fireEvent.click(screen.getByRole("button"));

    expect(onNavigate).toHaveBeenCalledWith("session-1", 30000);
    expect(pendingSeek.requestSeek).not.toHaveBeenCalled();
  });

  it("renders timestamp variant as ▸ mm:ss and seeks the active session", () => {
    const seekListener = vi.fn();
    window.addEventListener("notare:seek", seekListener);

    useTabs.setState({
      currentTab: {
        type: "sessions",
        id: "active-session",
        state: { view: null, autoStart: false },
        active: true,
        slotId: "default",
        pinned: false,
      },
    });

    render(<CitationChip ms={75000} variant="timestamp" />);

    const button = screen.getByRole("button");
    expect(button.textContent).toContain("▸");
    expect(button.textContent).toContain("01:15");

    fireEvent.click(button);

    expect(pendingSeek.requestSeek).toHaveBeenCalledWith(
      "active-session",
      75000,
    );
    expect(seekListener).toHaveBeenCalledTimes(1);
    const event = seekListener.mock.calls[0][0] as CustomEvent;
    expect(event.detail).toEqual({ ms: 75000, sessionId: "active-session" });

    window.removeEventListener("notare:seek", seekListener);
  });
});

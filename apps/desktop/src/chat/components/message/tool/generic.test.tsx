import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  requestSeek: vi.fn(),
  sessions: [
    {
      id: "session-1",
      title: "Team Sync",
      created_at: "2026-11-05T10:00:00Z",
    },
  ],
}));

vi.mock("~/session/queries", () => ({
  useSessionSummaries: () => mocks.sessions,
}));

vi.mock("~/audio-player/pending-seek", () => ({
  requestSeek: mocks.requestSeek,
}));

const mockTabsState = {
  currentTab: { type: "sessions", id: "session-1", state: {} },
  openNew: vi.fn(),
};

vi.mock("~/store/zustand/tabs", () => {
  const useTabsMock = (selector?: (state: typeof mockTabsState) => unknown) =>
    selector ? selector(mockTabsState) : mockTabsState;
  useTabsMock.getState = () => mockTabsState;
  return { useTabs: useTabsMock };
});

vi.mock("~/chat/hooks/use-chat-appearance", () => ({
  useChatAppearance: () => ({
    isDarkAppearance: false,
    toolbarSurface: "light",
    panelClassName: "bg-card text-foreground",
    panelBorderClassName: "border-border",
    elevatedSurfaceClassName: "bg-muted text-foreground",
    inputEditorClassName: "text-foreground",
  }),
}));

import { ToolGeneric, ToolOutputText } from "./generic";

describe("ToolGeneric output citations", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  afterEach(() => {
    cleanup();
  });

  it("renders [mm:ss] timestamps as clickable citation chips", () => {
    const seekListener = vi.fn();
    window.addEventListener("notare:seek", seekListener);

    render(
      <ToolOutputText text="Key moment discussed at [01:15] in the meeting." />,
    );

    expect(screen.getByText("Key moment discussed at")).toBeDefined();
    const chip = screen.getByRole("button");
    expect(chip.textContent).toContain("▸");
    expect(chip.textContent).toContain("01:15");

    fireEvent.click(chip);

    expect(seekListener).toHaveBeenCalled();
    const event = seekListener.mock.calls[0][0] as CustomEvent;
    expect(event.detail).toEqual({ ms: 75000, sessionId: "session-1" });
    expect(mocks.requestSeek).toHaveBeenCalledWith("session-1", 75000);

    window.removeEventListener("notare:seek", seekListener);
  });

  it("renders at_ms timestamps as clickable citation chips", () => {
    render(<ToolOutputText text='Found action item at "at_ms": 120000.' />);

    const chip = screen.getByRole("button");
    expect(chip.textContent).toContain("▸");
    expect(chip.textContent).toContain("02:00");
  });

  it("renders tool part output using ToolGeneric", () => {
    const part = {
      type: "tool-search_transcripts",
      state: "output-available",
      output: {
        excerpt: "Decision made at [03:45]",
      },
    };

    render(<ToolGeneric part={part} />);

    const chip = screen.getByRole("button", { name: /03:45/ });
    expect(chip).toBeDefined();
    expect(chip.textContent).toContain("03:45");
  });
});

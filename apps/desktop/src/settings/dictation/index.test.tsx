import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import {
  ActivationModeGroup,
  CleanupGroup,
  DictationHistoryList,
  DictationHistorySection,
  MacosAccessibilityHint,
  normalizeActivationMode,
  OutputModeGroup,
  WarmMicRow,
} from "./index";

import type { DictationHistoryEntry } from "~/dictation/history";

const mocks = vi.hoisted(() => ({
  ask: vi.fn(),
  toastError: vi.fn(),
  clearDictationHistory: vi.fn(),
  history: [] as DictationHistoryEntry[],
}));

vi.mock("@tauri-apps/plugin-dialog", async (importOriginal) => ({
  ...(await importOriginal<typeof import("@tauri-apps/plugin-dialog")>()),
  ask: mocks.ask,
}));

vi.mock("@hypr/ui/components/ui/toast", async (importOriginal) => ({
  ...(await importOriginal<typeof import("@hypr/ui/components/ui/toast")>()),
  sonnerToast: { error: mocks.toastError, success: vi.fn() },
}));

vi.mock("~/dictation/history", async (importOriginal) => ({
  ...(await importOriginal<typeof import("~/dictation/history")>()),
  useDictationHistory: () => mocks.history,
  clearDictationHistory: mocks.clearDictationHistory,
}));

describe("OutputModeGroup", () => {
  afterEach(() => {
    cleanup();
  });

  it("selects the current mode and emits the new one", () => {
    const onChange = vi.fn();
    render(<OutputModeGroup value="type" onChange={onChange} />);

    const type = screen.getByRole("radio", { name: /Type as you speak/ });
    const batch = screen.getByRole("radio", {
      name: /Collect and deliver when you stop/,
    });
    expect((type as HTMLInputElement).checked).toBe(true);
    expect((batch as HTMLInputElement).checked).toBe(false);

    fireEvent.click(batch);
    expect(onChange).toHaveBeenCalledWith("batch");
  });
});

describe("CleanupGroup", () => {
  afterEach(() => {
    cleanup();
  });

  it("offers none, basic and AI cleanup", () => {
    const onChange = vi.fn();
    render(<CleanupGroup value="basic" onChange={onChange} />);

    expect(
      (screen.getByRole("radio", { name: /Basic/ }) as HTMLInputElement)
        .checked,
    ).toBe(true);

    fireEvent.click(screen.getByRole("radio", { name: /AI cleanup/ }));
    expect(onChange).toHaveBeenCalledWith("llm");

    fireEvent.click(screen.getByRole("radio", { name: /None/ }));
    expect(onChange).toHaveBeenCalledWith("none");
  });
});

describe("normalizeActivationMode", () => {
  it('passes through known modes and falls back to "toggle"', () => {
    expect(normalizeActivationMode("toggle")).toBe("toggle");
    expect(normalizeActivationMode("push_to_talk")).toBe("push_to_talk");
    expect(normalizeActivationMode(undefined)).toBe("toggle");
    expect(normalizeActivationMode("bogus")).toBe("toggle");
  });
});

describe("ActivationModeGroup", () => {
  afterEach(() => {
    cleanup();
  });

  it("selects the current mode and emits the new one", () => {
    const onChange = vi.fn();
    render(<ActivationModeGroup value="toggle" onChange={onChange} />);

    const toggle = screen.getByRole("radio", { name: /Toggle press/ });
    const holdToTalk = screen.getByRole("radio", { name: /Hold to talk/ });
    expect((toggle as HTMLInputElement).checked).toBe(true);
    expect((holdToTalk as HTMLInputElement).checked).toBe(false);

    fireEvent.click(holdToTalk);
    expect(onChange).toHaveBeenCalledWith("push_to_talk");
  });

  it("shows the hold-to-talk release copy", () => {
    render(<ActivationModeGroup value="push_to_talk" onChange={vi.fn()} />);

    expect(
      screen.getByText(
        /Hold the dictation shortcut to talk; release to stop and deliver\./,
      ),
    ).toBeTruthy();
  });
});

describe("DictationHistoryList", () => {
  // Pin the clock so the relative-timestamp assertions are deterministic and
  // don't flake under CI load (the entries use absolute offsets from `now`).
  const NOW = new Date("2026-07-17T12:00:00.000Z").getTime();

  beforeEach(() => {
    vi.useFakeTimers();
    vi.setSystemTime(NOW);
  });
  afterEach(() => {
    vi.useRealTimers();
    cleanup();
  });

  const entries: DictationHistoryEntry[] = [
    {
      id: "one",
      text: "First dictation",
      rawText: "first dictation",
      source: "dictation",
      model: null,
      durationMs: null,
      pinned: false,
      status: "delivered",
      createdAt: new Date(NOW - 30_000).toISOString(),
    },
    {
      id: "two",
      text: "Second dictation",
      rawText: null,
      source: "dictation",
      model: null,
      durationMs: null,
      pinned: false,
      status: "delivered",
      createdAt: new Date(NOW - 90_000).toISOString(),
    },
  ];

  it("shows an empty state without entries", () => {
    render(
      <DictationHistoryList entries={[]} onCopy={vi.fn()} onDelete={vi.fn()} />,
    );

    expect(screen.queryByTestId("dictation-history-list")).toBeNull();
    expect(screen.getByText(/Nothing here yet/)).not.toBeNull();
  });

  it("copies an entry on click and deletes via the row button", () => {
    const onCopy = vi.fn();
    const onDelete = vi.fn();
    render(
      <DictationHistoryList
        entries={entries}
        onCopy={onCopy}
        onDelete={onDelete}
      />,
    );

    fireEvent.click(screen.getByText("First dictation"));
    expect(onCopy).toHaveBeenCalledWith(entries[0]);

    const deleteButtons = screen.getAllByRole("button", {
      name: "Delete history entry",
    });
    expect(deleteButtons).toHaveLength(2);
    fireEvent.click(deleteButtons[1]);
    expect(onDelete).toHaveBeenCalledWith(entries[1]);
  });

  it("shows a relative timestamp for each entry", () => {
    render(
      <DictationHistoryList
        entries={entries}
        onCopy={vi.fn()}
        onDelete={vi.fn()}
      />,
    );

    expect(screen.getAllByText(/ago$/)).toHaveLength(2);
  });
});

describe("DictationHistorySection clear all", () => {
  beforeEach(() => {
    vi.spyOn(console, "error").mockImplementation(() => {});
    mocks.history = [
      {
        id: "one",
        text: "First dictation",
        rawText: null,
        source: "dictation",
        model: null,
        durationMs: null,
        pinned: false,
        status: "delivered",
        createdAt: new Date().toISOString(),
      },
    ];
  });
  afterEach(() => {
    cleanup();
    vi.clearAllMocks();
    vi.restoreAllMocks();
    mocks.history = [];
  });

  const clickClearAll = () => {
    render(
      <DictationHistorySection retention="" onRetentionChange={vi.fn()} />,
    );
    fireEvent.click(screen.getByRole("button", { name: "Clear all" }));
  };

  it("keeps the history when the confirmation is declined", async () => {
    mocks.ask.mockResolvedValue(false);

    clickClearAll();

    await waitFor(() => expect(mocks.ask).toHaveBeenCalledTimes(1));
    expect(mocks.clearDictationHistory).not.toHaveBeenCalled();
  });

  it("clears the history once confirmed", async () => {
    mocks.ask.mockResolvedValue(true);
    mocks.clearDictationHistory.mockResolvedValue(undefined);

    clickClearAll();

    await waitFor(() =>
      expect(mocks.clearDictationHistory).toHaveBeenCalledTimes(1),
    );
    expect(mocks.toastError).not.toHaveBeenCalled();
  });

  it("reports a failed clear", async () => {
    mocks.ask.mockResolvedValue(true);
    mocks.clearDictationHistory.mockRejectedValue(new Error("db is locked"));

    clickClearAll();

    await waitFor(() => expect(mocks.toastError).toHaveBeenCalledTimes(1));
  });
});

// Lane B2: the warm-mic toggle. OFF by default and privacy-sensitive - the
// copy must spell out that the mic stays open and the OS indicator may show.
describe("WarmMicRow", () => {
  afterEach(() => {
    cleanup();
  });

  it("reflects the current value and emits the toggled one", () => {
    const onChange = vi.fn();
    render(<WarmMicRow checked={false} onChange={onChange} />);

    const toggle = screen.getByRole("switch");
    expect(toggle.getAttribute("aria-checked")).toBe("false");

    fireEvent.click(toggle);
    expect(onChange).toHaveBeenCalledWith(true);
  });

  it("states the privacy trade-off plainly", () => {
    render(<WarmMicRow checked={false} onChange={vi.fn()} />);

    expect(screen.getByText(/microphone open while idle/i)).toBeTruthy();
    expect(screen.getByText(/mic-in-use indicator/i)).toBeTruthy();
  });
});

// #31: paste-at-cursor / type-as-you-speak on macOS both need the
// Accessibility permission (enigo's CGEvent path) - this inline hint is the
// contextual nudge shown in the Dictation settings while it is missing.
describe("MacosAccessibilityHint", () => {
  afterEach(() => {
    cleanup();
  });

  it("renders nothing once accessibility is authorized", () => {
    const { container } = render(
      <MacosAccessibilityHint
        status="authorized"
        isPending={false}
        onRequest={vi.fn()}
        onOpen={vi.fn()}
      />,
    );

    expect(container.firstChild).toBeNull();
  });

  it("offers to request access when never asked", () => {
    const onRequest = vi.fn();
    render(
      <MacosAccessibilityHint
        status="neverRequested"
        isPending={false}
        onRequest={onRequest}
        onOpen={vi.fn()}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: /Grant access/ }));
    expect(onRequest).toHaveBeenCalledTimes(1);
  });

  it("offers to open Settings once denied", () => {
    const onOpen = vi.fn();
    render(
      <MacosAccessibilityHint
        status="denied"
        isPending={false}
        onRequest={vi.fn()}
        onOpen={onOpen}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: /Open Settings/ }));
    expect(onOpen).toHaveBeenCalledTimes(1);
  });
});

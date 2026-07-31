import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
} from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { addHours, startOfDay, subDays } from "@hypr/utils";

import type { DictationHistoryEntry } from "~/dictation/history";

type DeliverTextResult =
  | { status: "ok"; data: null }
  | { status: "error"; error: string };

const mocks = vi.hoisted(() => ({
  writeText: vi.fn(async () => undefined),
  deliverText: vi.fn(
    async (): Promise<DeliverTextResult> => ({ status: "ok", data: null }),
  ),
  sonnerSuccess: vi.fn(),
  sonnerError: vi.fn(),
  useSnippetsHistoryQuery: vi.fn(),
  setPinnedMutate: vi.fn(),
  deleteMutate: vi.fn(),
  updateTextMutate: vi.fn(
    (
      _vars: { id: string; text: string },
      opts?: { onSuccess?: () => void; onError?: () => void },
    ) => opts?.onSuccess?.(),
  ),
  addSuggestedDictionaryMappings: vi.fn(async () => ({
    added: [] as { wrong: string; right: string; caseSensitive: boolean }[],
  })),
  showTransientToast: vi.fn(),
}));

vi.mock("@tauri-apps/plugin-clipboard-manager", () => ({
  writeText: mocks.writeText,
}));

vi.mock("@hypr/plugin-dictation", () => ({
  commands: { deliverText: mocks.deliverText },
}));

vi.mock("@hypr/ui/components/ui/toast", () => ({
  sonnerToast: { success: mocks.sonnerSuccess, error: mocks.sonnerError },
}));

vi.mock("~/shared/main", () => ({
  StandardContentWrapper: ({ children }: { children: React.ReactNode }) => (
    <>{children}</>
  ),
}));

vi.mock("./queries", () => ({
  useSnippetsHistoryQuery: (query: string) =>
    mocks.useSnippetsHistoryQuery(query),
  useSetSnippetPinned: () => ({ mutate: mocks.setPinnedMutate }),
  useDeleteSnippet: () => ({ mutate: mocks.deleteMutate }),
  useUpdateSnippetText: () => ({ mutate: mocks.updateTextMutate }),
  addSuggestedDictionaryMappings: (
    ...args: Parameters<typeof mocks.addSuggestedDictionaryMappings>
  ) => mocks.addSuggestedDictionaryMappings(...args),
}));

vi.mock("~/sidebar/toast/transient", () => ({
  showTransientToast: (...args: unknown[]) => mocks.showTransientToast(...args),
}));

import { TabContentSnippets } from "./index";

function makeEntry(
  overrides: Partial<DictationHistoryEntry> & { id: string; createdAt: string },
): DictationHistoryEntry {
  return {
    text: "hello",
    rawText: null,
    source: "dictation",
    model: null,
    durationMs: null,
    pinned: false,
    status: "delivered",
    ...overrides,
  };
}

function localTimeOn(daysAgo: number, hour = 9): string {
  return addHours(startOfDay(subDays(new Date(), daysAgo)), hour).toISOString();
}

interface QueryResult {
  data:
    | {
        pages: {
          entries: DictationHistoryEntry[];
          nextCursor: string | null;
        }[];
      }
    | undefined;
  isLoading: boolean;
  isError: boolean;
  hasNextPage: boolean;
  isFetchingNextPage: boolean;
  fetchNextPage: () => void;
}

function defaultQueryResult(
  entries: DictationHistoryEntry[] = [],
): QueryResult {
  return {
    data: { pages: [{ entries, nextCursor: null }] },
    isLoading: false,
    isError: false,
    hasNextPage: false,
    isFetchingNextPage: false,
    fetchNextPage: vi.fn(),
  };
}

function setQueryResult(result: QueryResult) {
  mocks.useSnippetsHistoryQuery.mockReturnValue(result);
}

describe("TabContentSnippets", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.deliverText.mockResolvedValue({ status: "ok", data: null });
    setQueryResult(defaultQueryResult());
  });

  afterEach(() => {
    cleanup();
  });

  it("shows a loading skeleton while the first page is loading", () => {
    setQueryResult({
      ...defaultQueryResult(),
      isLoading: true,
      data: undefined,
    });

    render(<TabContentSnippets />);

    expect(screen.getByTestId("snippets-loading-skeleton")).toBeTruthy();
  });

  it("shows the 'no history yet' empty state when there are no entries and no search", () => {
    render(<TabContentSnippets />);

    expect(screen.getByText("No snippets yet")).toBeTruthy();
  });

  it("shows the 'no results' empty state when a search has no matches", () => {
    vi.useFakeTimers();
    render(<TabContentSnippets />);

    fireEvent.change(
      screen.getByPlaceholderText("Search your dictation history..."),
      { target: { value: "xyz" } },
    );
    act(() => {
      vi.advanceTimersByTime(300);
    });

    expect(screen.getByText(/No results/)).toBeTruthy();
    vi.useRealTimers();
  });

  it("debounces the search box and forwards the trimmed-by-caller query to the history hook", () => {
    vi.useFakeTimers();
    render(<TabContentSnippets />);

    fireEvent.change(
      screen.getByPlaceholderText("Search your dictation history..."),
      { target: { value: "standup notes" } },
    );
    // Not yet debounced.
    expect(mocks.useSnippetsHistoryQuery).not.toHaveBeenLastCalledWith(
      "standup notes",
    );

    act(() => {
      vi.advanceTimersByTime(300);
    });

    expect(mocks.useSnippetsHistoryQuery).toHaveBeenLastCalledWith(
      "standup notes",
    );
    vi.useRealTimers();
  });

  it("groups entries into a pinned section and day buckets, newest-first within each", () => {
    const pinned = makeEntry({
      id: "pinned-1",
      text: "pinned snippet",
      pinned: true,
      createdAt: localTimeOn(0, 10),
    });
    const todayNewer = makeEntry({
      id: "today-newer",
      text: "today newer",
      createdAt: localTimeOn(0, 11),
    });
    const todayOlder = makeEntry({
      id: "today-older",
      text: "today older",
      createdAt: localTimeOn(0, 8),
    });
    const yesterday = makeEntry({
      id: "yesterday-1",
      text: "yesterday snippet",
      createdAt: localTimeOn(1, 9),
    });

    setQueryResult(
      defaultQueryResult([pinned, todayNewer, todayOlder, yesterday]),
    );
    render(<TabContentSnippets />);

    expect(screen.getByText("Pinned")).toBeTruthy();
    expect(screen.getByText("Today")).toBeTruthy();
    expect(screen.getByText("Yesterday")).toBeTruthy();
    expect(screen.getByTestId("snippets-pinned-list").textContent).toContain(
      "pinned snippet",
    );

    const rows = screen.getAllByTestId("snippet-entry");
    expect(rows.map((row) => row.getAttribute("data-entry-id"))).toEqual([
      "pinned-1",
      "today-newer",
      "today-older",
      "yesterday-1",
    ]);
  });

  it("renders discarded entries with a Discarded badge and a muted style", () => {
    const discarded = makeEntry({
      id: "discarded-1",
      text: "a recovered dictation",
      status: "discarded",
      createdAt: localTimeOn(0),
    });
    setQueryResult(defaultQueryResult([discarded]));

    render(<TabContentSnippets />);

    const row = screen.getByTestId("snippet-entry");
    expect(row.dataset.discarded).toBe("true");
    expect(screen.getByText("Discarded")).toBeTruthy();
  });

  it("copies an entry's cleaned text and shows a success toast", async () => {
    const entry = makeEntry({
      id: "e1",
      text: "copy me",
      createdAt: localTimeOn(0),
    });
    setQueryResult(defaultQueryResult([entry]));
    render(<TabContentSnippets />);

    fireEvent.click(screen.getByRole("button", { name: "Copy" }));

    await act(async () => {
      await Promise.resolve();
    });

    expect(mocks.writeText).toHaveBeenCalledWith("copy me");
    expect(mocks.sonnerSuccess).toHaveBeenCalledWith("Copied to clipboard");
  });

  it("inserts an entry at the cursor via deliverText(text, true)", async () => {
    const entry = makeEntry({
      id: "e1",
      text: "insert me",
      createdAt: localTimeOn(0),
    });
    setQueryResult(defaultQueryResult([entry]));
    render(<TabContentSnippets />);

    fireEvent.click(screen.getByRole("button", { name: "Insert at cursor" }));

    await act(async () => {
      await Promise.resolve();
    });

    expect(mocks.deliverText).toHaveBeenCalledWith("insert me", true);
    expect(mocks.sonnerError).not.toHaveBeenCalled();
  });

  it("shows an error toast when insert-at-cursor fails", async () => {
    mocks.deliverText.mockResolvedValueOnce({
      status: "error",
      error: "no focused app",
    });
    const entry = makeEntry({
      id: "e1",
      text: "fails",
      createdAt: localTimeOn(0),
    });
    setQueryResult(defaultQueryResult([entry]));
    render(<TabContentSnippets />);

    fireEvent.click(screen.getByRole("button", { name: "Insert at cursor" }));

    await act(async () => {
      await Promise.resolve();
    });

    expect(mocks.sonnerError).toHaveBeenCalledWith("Couldn't insert text");
  });

  it("toggles pin state via the pin/unpin action", () => {
    const entry = makeEntry({
      id: "e1",
      text: "pin me",
      pinned: false,
      createdAt: localTimeOn(0),
    });
    setQueryResult(defaultQueryResult([entry]));
    render(<TabContentSnippets />);

    fireEvent.click(screen.getByRole("button", { name: "Pin" }));

    expect(mocks.setPinnedMutate).toHaveBeenCalledWith(
      { id: "e1", pinned: true },
      expect.objectContaining({ onError: expect.any(Function) }),
    );
  });

  it("deletes an entry via the delete action", () => {
    const entry = makeEntry({
      id: "e1",
      text: "delete me",
      createdAt: localTimeOn(0),
    });
    setQueryResult(defaultQueryResult([entry]));
    render(<TabContentSnippets />);

    fireEvent.click(screen.getByRole("button", { name: "Delete" }));

    expect(mocks.deleteMutate).toHaveBeenCalledWith(
      "e1",
      expect.objectContaining({ onError: expect.any(Function) }),
    );
  });

  it("shows the meeting badge for meeting-sourced entries", () => {
    const entry = makeEntry({
      id: "e1",
      text: "from a meeting",
      source: "meeting",
      createdAt: localTimeOn(0),
    });
    setQueryResult(defaultQueryResult([entry]));

    render(<TabContentSnippets />);

    expect(screen.getByText("Meeting")).toBeTruthy();
    expect(screen.queryByText("Dictation")).toBeNull();
  });

  it("expands the raw transcript on demand when it differs from the cleaned text", () => {
    const entry = makeEntry({
      id: "e1",
      text: "cleaned text",
      rawText: "uh raw text uh",
      createdAt: localTimeOn(0),
    });
    setQueryResult(defaultQueryResult([entry]));

    render(<TabContentSnippets />);

    expect(screen.queryByText("uh raw text uh")).toBeNull();

    fireEvent.click(screen.getByRole("button", { name: /Raw transcript/ }));

    expect(screen.getByText("uh raw text uh")).toBeTruthy();
  });

  it("does not offer a raw-transcript toggle when rawText matches the cleaned text", () => {
    const entry = makeEntry({
      id: "e1",
      text: "same text",
      rawText: "same text",
      createdAt: localTimeOn(0),
    });
    setQueryResult(defaultQueryResult([entry]));

    render(<TabContentSnippets />);

    expect(screen.queryByRole("button", { name: /Raw transcript/ })).toBeNull();
  });

  it("edits a snippet's text inline and saves it", async () => {
    const entry = makeEntry({
      id: "e1",
      text: "hello wrld",
      createdAt: localTimeOn(0),
    });
    setQueryResult(defaultQueryResult([entry]));
    render(<TabContentSnippets />);

    fireEvent.click(screen.getByRole("button", { name: "Edit" }));
    const textarea = screen.getByTestId("snippet-entry-edit-textarea");
    fireEvent.change(textarea, { target: { value: "hello world" } });
    fireEvent.click(screen.getByRole("button", { name: "Save" }));

    await act(async () => {
      await Promise.resolve();
    });

    expect(mocks.updateTextMutate).toHaveBeenCalledWith(
      { id: "e1", text: "hello world" },
      expect.objectContaining({
        onSuccess: expect.any(Function),
        onError: expect.any(Function),
      }),
    );
    // Back to display mode with the (mutation-optimistic) draft no longer shown.
    expect(screen.queryByTestId("snippet-entry-edit-textarea")).toBeNull();
  });

  it("cancels an inline edit without saving", () => {
    const entry = makeEntry({
      id: "e1",
      text: "hello",
      createdAt: localTimeOn(0),
    });
    setQueryResult(defaultQueryResult([entry]));
    render(<TabContentSnippets />);

    fireEvent.click(screen.getByRole("button", { name: "Edit" }));
    fireEvent.change(screen.getByTestId("snippet-entry-edit-textarea"), {
      target: { value: "discarded draft" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));

    expect(mocks.updateTextMutate).not.toHaveBeenCalled();
    expect(screen.queryByTestId("snippet-entry-edit-textarea")).toBeNull();
    expect(screen.getByText("hello")).toBeTruthy();
  });

  it("offers a dictionary suggestion toast after saving an edit with a term-like diff", async () => {
    const entry = makeEntry({
      id: "e1",
      text: "talked to far eye about it",
      createdAt: localTimeOn(0),
    });
    setQueryResult(defaultQueryResult([entry]));
    render(<TabContentSnippets />);

    fireEvent.click(screen.getByRole("button", { name: "Edit" }));
    fireEvent.change(screen.getByTestId("snippet-entry-edit-textarea"), {
      target: { value: "talked to FarEye about it" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Save" }));

    await act(async () => {
      await Promise.resolve();
    });

    expect(mocks.showTransientToast).toHaveBeenCalledTimes(1);
    const [toast] = mocks.showTransientToast.mock.calls[0];
    expect(toast.description).toContain("far eye");
    expect(toast.description).toContain("FarEye");
    expect(toast.dismissible).toBe(true);

    await toast.primaryAction.onClick();
    expect(mocks.addSuggestedDictionaryMappings).toHaveBeenCalledWith([
      { wrong: "far eye", right: "FarEye" },
    ]);
  });

  it("does not offer a suggestion toast when the edit has no term-like diff", async () => {
    const entry = makeEntry({
      id: "e1",
      text: "a short note",
      createdAt: localTimeOn(0),
    });
    setQueryResult(defaultQueryResult([entry]));
    render(<TabContentSnippets />);

    fireEvent.click(screen.getByRole("button", { name: "Edit" }));
    fireEvent.change(screen.getByTestId("snippet-entry-edit-textarea"), {
      target: { value: "a short note" },
    });
    // Unchanged text: Save is disabled, nothing should fire.
    expect(
      (screen.getByRole("button", { name: "Save" }) as HTMLButtonElement)
        .disabled,
    ).toBe(true);
  });

  it("shows a Load more button when another page is available and calls fetchNextPage", () => {
    const entry = makeEntry({ id: "e1", createdAt: localTimeOn(0) });
    const fetchNextPage = vi.fn();
    setQueryResult({
      ...defaultQueryResult([entry]),
      hasNextPage: true,
      fetchNextPage,
    });
    render(<TabContentSnippets />);

    fireEvent.click(screen.getByRole("button", { name: "Load more" }));

    expect(fetchNextPage).toHaveBeenCalled();
  });
});

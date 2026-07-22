import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";

import {
  CustomSttModelSection,
  PROGRESS_MAX_CONSECUTIVE_FAILURES,
} from "./select";

const { listMock, downloadMock, activateMock, progressMock } = vi.hoisted(
  () => ({
    listMock: vi.fn(),
    downloadMock: vi.fn(),
    activateMock: vi.fn(),
    progressMock: vi.fn(),
  }),
);

vi.mock("./list-custom-stt", () => ({
  listCustomSttModels: listMock,
  downloadCustomSttModel: downloadMock,
  activateCustomSttModel: activateMock,
  fetchCustomSttModelProgress: progressMock,
}));

const INSTALLED_MODEL = {
  id: "QuantizedLargeTurbo",
  displayName: "Large v3 Turbo (Q8)",
  description: "Big and accurate",
  sizeBytes: 874000000,
  englishOnly: false,
  active: false,
  installed: true,
  corrupt: false,
  unknown: false,
};

const NOT_INSTALLED_MODEL = {
  id: "QuantizedSmall",
  displayName: "Small",
  description: "Tiny",
  sizeBytes: 480000000,
  englishOnly: true,
  active: false,
  installed: false,
  corrupt: false,
  unknown: false,
};

function okModels(models: (typeof INSTALLED_MODEL)[]) {
  return { ok: true as const, models };
}

// Track the active QueryClient so afterEach can tear it down. Without this,
// background refetches an activate/download kicks off (invalidateQueries /
// refetch) leak past cleanup() and fire during the next test with a reset mock,
// crashing the next render.
let currentClient: QueryClient | null = null;

function makeClient() {
  currentClient = new QueryClient({
    defaultOptions: { queries: { retry: false, gcTime: 0 } },
  });
  return currentClient;
}

function renderSection(
  models: (typeof INSTALLED_MODEL)[],
  onSelect: (id: string) => void = vi.fn(),
) {
  listMock.mockResolvedValue(okModels(models));
  return render(
    <QueryClientProvider client={makeClient()}>
      <CustomSttModelSection
        baseUrl="http://192.168.0.91:8383/v1"
        apiKey=""
        selectedId=""
        onSelect={onSelect}
      />
    </QueryClientProvider>,
  );
}

afterEach(() => {
  currentClient?.clear();
  currentClient = null;
  cleanup();
  listMock.mockReset();
  downloadMock.mockReset();
  activateMock.mockReset();
  progressMock.mockReset();
  vi.useRealTimers();
});

describe("CustomSttModelSection", () => {
  test("renders the fetched model list", async () => {
    renderSection([INSTALLED_MODEL, NOT_INSTALLED_MODEL]);

    await waitFor(() => {
      expect(screen.getByText("Large v3 Turbo (Q8)")).toBeTruthy();
    });
    expect(screen.getByText("Small")).toBeTruthy();
  });

  test("an installed, inactive model shows Activate (not Download)", async () => {
    renderSection([INSTALLED_MODEL]);

    await waitFor(() => {
      expect(screen.getByRole("button", { name: /activate/i })).toBeTruthy();
    });
    expect(screen.queryByRole("button", { name: /download/i })).toBeNull();
  });

  test("clicking Activate calls activate and selects the model once confirmed active", async () => {
    const onSelect = vi.fn();
    // First call = the list query (model inactive, so the Activate button
    // shows). Second call = the post-activate verification refetch, which
    // reports the model as active. Later calls (invalidate refetch) keep it.
    listMock
      .mockResolvedValueOnce(okModels([{ ...INSTALLED_MODEL, active: false }]))
      .mockResolvedValue(okModels([{ ...INSTALLED_MODEL, active: true }]));
    activateMock.mockResolvedValue({ ok: true });

    render(
      <QueryClientProvider client={makeClient()}>
        <CustomSttModelSection
          baseUrl="http://192.168.0.91:8383/v1"
          apiKey=""
          selectedId=""
          onSelect={onSelect}
        />
      </QueryClientProvider>,
    );

    const activate = await screen.findByRole("button", { name: /activate/i });
    fireEvent.click(activate);

    await waitFor(() => {
      expect(activateMock).toHaveBeenCalledWith(
        "http://192.168.0.91:8383/v1",
        "",
        "QuantizedLargeTurbo",
      );
    });
    await waitFor(() => {
      expect(onSelect).toHaveBeenCalledWith("QuantizedLargeTurbo");
    });
  });

  // PG-3: a 200 from activate must not be trusted blindly. If the refetch
  // doesn't show the model active, surface a warning instead of selecting it.
  test("does not treat the model as active when the server doesn't confirm it", async () => {
    const onSelect = vi.fn();
    listMock.mockResolvedValue(
      okModels([{ ...INSTALLED_MODEL, active: false }]),
    );
    activateMock.mockResolvedValue({ ok: true });

    render(
      <QueryClientProvider client={makeClient()}>
        <CustomSttModelSection
          baseUrl="http://192.168.0.91:8383/v1"
          apiKey=""
          selectedId=""
          onSelect={onSelect}
        />
      </QueryClientProvider>,
    );

    const activate = await screen.findByRole("button", { name: /activate/i });
    fireEvent.click(activate);

    await waitFor(() => {
      expect(screen.getByText(/didn't confirm it as active/i)).toBeTruthy();
    });
    expect(onSelect).not.toHaveBeenCalled();
  });

  test("an un-installed model shows Download", async () => {
    renderSection([NOT_INSTALLED_MODEL]);

    await waitFor(() => {
      expect(screen.getByRole("button", { name: /download/i })).toBeTruthy();
    });
    expect(screen.queryByRole("button", { name: /activate/i })).toBeNull();
  });

  test("clicking Download starts the download and shows progress", async () => {
    // Keep progress in-flight (not complete, not failed) so the row stays in
    // the downloading state with a percent badge.
    progressMock.mockResolvedValue({
      percent: 42,
      complete: false,
      failed: false,
    });
    downloadMock.mockResolvedValue({ ok: true });

    renderSection([NOT_INSTALLED_MODEL]);

    const download = await screen.findByRole("button", { name: /download/i });
    fireEvent.click(download);

    await waitFor(() => {
      expect(downloadMock).toHaveBeenCalledWith(
        "http://192.168.0.91:8383/v1",
        "",
        "QuantizedSmall",
      );
    });
    await waitFor(() => {
      expect(screen.getByText("0%")).toBeTruthy();
    });
  });

  // PG-1: a model with unknown integrity renders a neutral Unknown badge and
  // no Download CTA (the server didn't tell us its state, so don't claim it
  // needs downloading).
  test("an unknown-integrity model shows Unknown and no Download/Activate", async () => {
    renderSection([{ ...NOT_INSTALLED_MODEL, unknown: true }]);

    await waitFor(() => {
      expect(screen.getByText(/unknown/i)).toBeTruthy();
    });
    expect(screen.queryByRole("button", { name: /download/i })).toBeNull();
    expect(screen.queryByRole("button", { name: /activate/i })).toBeNull();
  });
});

// PG-2: progress polling must give up after a bounded number of consecutive
// failed/null polls instead of recursing forever (a dead server after
// download-start would otherwise spin indefinitely).
describe("CustomSttModelRow download-progress polling", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  test(`stops after ${PROGRESS_MAX_CONSECUTIVE_FAILURES} consecutive null polls and surfaces a stalled error`, async () => {
    listMock.mockResolvedValue(okModels([NOT_INSTALLED_MODEL]));
    downloadMock.mockResolvedValue({ ok: true });
    progressMock.mockResolvedValue(null);

    render(
      <QueryClientProvider client={makeClient()}>
        <CustomSttModelSection
          baseUrl="http://192.168.0.91:8383/v1"
          apiKey=""
          selectedId=""
          onSelect={vi.fn()}
        />
      </QueryClientProvider>,
    );

    // Flush the initial list query (microtask) so the Download button renders.
    // findByRole/waitFor would hang under fake timers, so query synchronously.
    await act(async () => {
      await vi.advanceTimersByTimeAsync(0);
    });
    const download = screen.getByRole("button", { name: /download/i });
    fireEvent.click(download);

    // Each poll fires every 1500ms; drive all N failures forward. Wrapping in
    // act flushes the React state updates the async timer callbacks produce.
    await act(async () => {
      await vi.advanceTimersByTimeAsync(
        1500 * PROGRESS_MAX_CONSECUTIVE_FAILURES + 100,
      );
    });

    expect(screen.getByText(/download stalled/i)).toBeTruthy();
    expect(progressMock).toHaveBeenCalledTimes(
      PROGRESS_MAX_CONSECUTIVE_FAILURES,
    );

    // No further polls after giving up.
    const callsBefore = progressMock.mock.calls.length;
    await act(async () => {
      await vi.advanceTimersByTimeAsync(1500 * 3);
    });
    expect(progressMock.mock.calls.length).toBe(callsBefore);
  });
});

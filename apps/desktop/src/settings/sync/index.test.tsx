import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  syncStatus: vi.fn(),
  syncThisDevice: vi.fn(),
  syncListPeers: vi.fn(),
  syncAddPeer: vi.fn(),
  syncRemovePeer: vi.fn(),
}));

vi.mock("@hypr/plugin-db", () => ({
  syncStatus: mocks.syncStatus,
  syncThisDevice: mocks.syncThisDevice,
  syncListPeers: mocks.syncListPeers,
  syncAddPeer: mocks.syncAddPeer,
  syncRemovePeer: mocks.syncRemovePeer,
}));

vi.mock("@lingui/react/macro", () => ({
  Trans: ({ children }: { children?: ReactNode }) => <>{children}</>,
  useLingui: () => ({
    t: (input: TemplateStringsArray | string, ...values: unknown[]) => {
      if (typeof input === "string") {
        return input;
      }
      return (input as readonly string[]).reduce(
        (message: string, part: string, index: number) =>
          `${message}${part}${index < values.length ? String(values[index]) : ""}`,
        "",
      );
    },
  }),
}));

import { isValidFingerprint, SettingsSync } from "./index";

// 52 z-base-32 chars, the exact compact-form length sync_p2p::Fingerprint expects.
const VALID_FINGERPRINT =
  "ybndrfg8ejkmcpqxot1uwisza345h769ybndrfg8ejkmcpqxot1u";
const VALID_FINGERPRINT_GROUPED = "ybnd-rfg8-ejkm-cpqx-ot1u";

function renderPage() {
  const queryClient = new QueryClient({
    defaultOptions: {
      queries: { retry: false },
      mutations: { retry: false },
    },
  });

  return render(
    <QueryClientProvider client={queryClient}>
      <SettingsSync />
    </QueryClientProvider>,
  );
}

describe("isValidFingerprint", () => {
  it("accepts a 52-char compact z-base-32 fingerprint", () => {
    expect(isValidFingerprint(VALID_FINGERPRINT)).toBe(true);
  });

  it("accepts the grouped dashed form of a valid fingerprint", () => {
    const grouped = `${VALID_FINGERPRINT.slice(0, 4)}-${VALID_FINGERPRINT.slice(4, 8)}`;
    expect(isValidFingerprint(grouped + VALID_FINGERPRINT.slice(8))).toBe(true);
  });

  it("rejects the wrong length", () => {
    expect(isValidFingerprint(VALID_FINGERPRINT.slice(0, 51))).toBe(false);
    expect(isValidFingerprint(VALID_FINGERPRINT + "y")).toBe(false);
  });

  it("rejects characters outside the z-base-32 alphabet", () => {
    // 'l' is not in sync_p2p's alphabet (visually confusable with '1')
    const bad = "l" + VALID_FINGERPRINT.slice(1);
    expect(isValidFingerprint(bad)).toBe(false);
  });

  it("rejects empty input", () => {
    expect(isValidFingerprint("")).toBe(false);
  });
});

describe("SettingsSync", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  afterEach(() => {
    cleanup();
  });

  it("renders a calm explanatory state when sync is unavailable, without throwing", async () => {
    mocks.syncStatus.mockResolvedValue({ kind: "unavailable" });

    renderPage();

    await waitFor(() =>
      expect(
        screen.getByText(/Device sync isn't available in this build/),
      ).toBeTruthy(),
    );
    expect(mocks.syncThisDevice).not.toHaveBeenCalled();
    expect(mocks.syncListPeers).not.toHaveBeenCalled();
  });

  it("does not call sync_add_peer for an invalid fingerprint", async () => {
    mocks.syncStatus.mockResolvedValue({ kind: "live", running: false });
    mocks.syncThisDevice.mockResolvedValue(VALID_FINGERPRINT_GROUPED);
    mocks.syncListPeers.mockResolvedValue([]);

    renderPage();

    const fingerprintInput = await screen.findByLabelText("Peer fingerprint");
    fireEvent.change(fingerprintInput, {
      target: { value: "not-a-fingerprint" },
    });

    await waitFor(() =>
      expect(
        screen.getByText("Enter a valid 52-character fingerprint"),
      ).toBeTruthy(),
    );

    const addButton = screen.getByRole("button", { name: /Add device/ });
    expect((addButton as HTMLButtonElement).disabled).toBe(true);

    fireEvent.click(addButton);
    expect(mocks.syncAddPeer).not.toHaveBeenCalled();
  });

  it("adds a peer and refreshes the paired-devices list on success", async () => {
    mocks.syncStatus.mockResolvedValue({ kind: "live", running: false });
    mocks.syncThisDevice.mockResolvedValue(VALID_FINGERPRINT_GROUPED);
    mocks.syncListPeers.mockResolvedValueOnce([]).mockResolvedValueOnce([
      {
        nodeId: VALID_FINGERPRINT,
        fingerprint: VALID_FINGERPRINT_GROUPED,
        label: "Work laptop",
        addedAt: 1000,
        lastSeen: 0,
      },
    ]);
    mocks.syncAddPeer.mockResolvedValue(VALID_FINGERPRINT_GROUPED);

    renderPage();

    await waitFor(() =>
      expect(screen.getByText("No devices paired yet.")).toBeTruthy(),
    );

    // Grouped + mixed-case input: the component must normalize this to the
    // compact lowercase form before calling sync_add_peer, not forward it
    // as-is (the validator checks the normalized form; submission must match).
    const groupedMixedCase = `${VALID_FINGERPRINT.slice(0, 4).toUpperCase()}-${VALID_FINGERPRINT.slice(4)}`;
    fireEvent.change(screen.getByLabelText("Peer fingerprint"), {
      target: { value: groupedMixedCase },
    });
    fireEvent.change(screen.getByLabelText("Device label"), {
      target: { value: "Work laptop" },
    });

    const addButton = await screen.findByRole("button", {
      name: /Add device/,
    });
    await waitFor(() =>
      expect((addButton as HTMLButtonElement).disabled).toBe(false),
    );
    fireEvent.click(addButton);

    await waitFor(() =>
      expect(mocks.syncAddPeer).toHaveBeenCalledWith(
        VALID_FINGERPRINT,
        "Work laptop",
      ),
    );
    await waitFor(() => expect(mocks.syncListPeers).toHaveBeenCalledTimes(2));
    await waitFor(() => expect(screen.getByText("Work laptop")).toBeTruthy());
  });

  it("requires confirmation before removing a paired device, then drops it from the list", async () => {
    mocks.syncStatus.mockResolvedValue({ kind: "live", running: false });
    mocks.syncThisDevice.mockResolvedValue(VALID_FINGERPRINT_GROUPED);
    mocks.syncListPeers
      .mockResolvedValueOnce([
        {
          nodeId: VALID_FINGERPRINT,
          fingerprint: VALID_FINGERPRINT_GROUPED,
          label: "Work laptop",
          addedAt: 1000,
          lastSeen: 0,
        },
      ])
      .mockResolvedValueOnce([]);
    mocks.syncRemovePeer.mockResolvedValue(true);

    renderPage();

    const removeButton = await screen.findByRole("button", {
      name: /Remove/,
    });
    fireEvent.click(removeButton);

    expect(screen.getByText("Remove this device?")).toBeTruthy();
    expect(mocks.syncRemovePeer).not.toHaveBeenCalled();

    fireEvent.click(
      within(screen.getByRole("dialog")).getByRole("button", {
        name: "Remove",
      }),
    );

    await waitFor(() =>
      expect(mocks.syncRemovePeer).toHaveBeenCalledWith(VALID_FINGERPRINT),
    );
    await waitFor(() => expect(screen.queryByText("Work laptop")).toBeNull());
    await waitFor(() =>
      expect(screen.getByText("No devices paired yet.")).toBeTruthy(),
    );
  });
});

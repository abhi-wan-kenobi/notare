import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import {
  act,
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
  syncRevokePeer: vi.fn(),
  syncStart: vi.fn(),
  syncStop: vi.fn(),
  pairingCreateOffer: vi.fn(),
  pairingJoin: vi.fn(),
  pairingStatus: vi.fn(),
  pairingConfirm: vi.fn(),
  execute: vi.fn(),
  openPath: vi.fn(),
  globalBase: vi.fn(),
  syncEnabled: undefined as boolean | undefined,
  setSyncEnabled: vi.fn(),
}));

vi.mock("@hypr/plugin-db", () => ({
  execute: mocks.execute,
  syncStatus: mocks.syncStatus,
  syncThisDevice: mocks.syncThisDevice,
  syncListPeers: mocks.syncListPeers,
  syncAddPeer: mocks.syncAddPeer,
  syncRevokePeer: mocks.syncRevokePeer,
  syncStart: mocks.syncStart,
  syncStop: mocks.syncStop,
  pairingCreateOffer: mocks.pairingCreateOffer,
  pairingJoin: mocks.pairingJoin,
  pairingStatus: mocks.pairingStatus,
  pairingConfirm: mocks.pairingConfirm,
}));

vi.mock("@hypr/plugin-opener2", () => ({
  commands: { openPath: mocks.openPath },
}));

vi.mock("@hypr/plugin-settings", () => ({
  commands: { globalBase: mocks.globalBase },
}));

vi.mock("~/settings/queries", () => ({
  useStoredSettingValue: (key: string) => ({
    value: key === "sync_enabled" ? mocks.syncEnabled : undefined,
    hasValue: mocks.syncEnabled !== undefined,
  }),
  useSetSettingValue: (key: string) => (value: unknown) => {
    if (key === "sync_enabled") {
      mocks.syncEnabled = value as boolean;
    }
    mocks.setSyncEnabled(key, value);
  },
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
import { usePendingPairLink } from "./pending-pair-link";

// 52 z-base-32 chars, the exact compact-form length sync_p2p::Fingerprint expects.
const VALID_FINGERPRINT =
  "ybndrfg8ejkmcpqxot1uwisza345h769ybndrfg8ejkmcpqxot1u";
const VALID_FINGERPRINT_GROUPED = "ybnd-rfg8-ejkm-cpqx-ot1u";
const OTHER_FINGERPRINT =
  "8ejkmcpqxot1uwisza345h769ybndrfg8ejkmcpqxot1uwisza34";

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

// The manual-fingerprint form lives under the collapsed "Advanced" accordion;
// expand it before interacting with "Peer fingerprint" / "Device label".
async function openAdvanced() {
  const trigger = await screen.findByText(
    "Advanced: manual fingerprint pairing",
  );
  fireEvent.click(trigger);
  await screen.findByLabelText("Peer fingerprint");
}

const ENABLE_SWITCH_NAME = "Enable device sync";
const BACKUP_PROMPT =
  "This changes how Notare stores notes so they can sync. Back up first?";

// A fresh database has no CRR tables, so the first enable goes through the
// backup prompt; confirm it.
async function confirmFirstEnable() {
  await screen.findByText(BACKUP_PROMPT);
  fireEvent.click(screen.getByRole("button", { name: "Continue" }));
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
    // Most of these tests exercise pairing (add/remove device) and predate
    // the sync_enabled runtime gate, so default it on here; the dedicated
    // "off" tests below set it explicitly.
    mocks.syncEnabled = true;
    mocks.syncStart.mockResolvedValue(undefined);
    mocks.syncStop.mockResolvedValue(undefined);
    // Fresh database: no synced table has been converted to a CRR yet.
    mocks.execute.mockResolvedValue([{ n: 0 }]);
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

  it("shows the engine load error and keeps the switch disabled when the engine is not loaded", async () => {
    mocks.syncEnabled = false;
    mocks.syncStatus.mockResolvedValue({
      kind: "live",
      engineLoaded: false,
      engineError:
        "failed to load extension: library load disallowed by system policy",
      running: false,
    });
    mocks.syncThisDevice.mockResolvedValue(VALID_FINGERPRINT_GROUPED);
    mocks.syncListPeers.mockResolvedValue([]);

    renderPage();

    await waitFor(() =>
      expect(screen.getByText(/The sync engine couldn't load/)).toBeTruthy(),
    );
    expect(
      screen.getByText(/library load disallowed by system policy/),
    ).toBeTruthy();
    const toggle = screen.getByRole("switch", { name: /Enable device sync/ });
    expect(toggle.hasAttribute("disabled")).toBe(true);
    fireEvent.click(toggle);
    expect(mocks.syncStart).not.toHaveBeenCalled();
  });

  describe("manual fingerprint pairing (Advanced)", () => {
    it("does not call sync_add_peer for an invalid fingerprint", async () => {
      mocks.syncStatus.mockResolvedValue({
        kind: "live",
        engineLoaded: true,
        running: false,
      });
      mocks.syncThisDevice.mockResolvedValue(VALID_FINGERPRINT_GROUPED);
      mocks.syncListPeers.mockResolvedValue([]);

      renderPage();
      await openAdvanced();

      const fingerprintInput = screen.getByLabelText("Peer fingerprint");
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
      mocks.syncStatus.mockResolvedValue({
        kind: "live",
        engineLoaded: true,
        running: false,
      });
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
      await openAdvanced();

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
  });

  it("requires confirmation before removing a paired device, then revokes it (not a soft remove)", async () => {
    mocks.syncStatus.mockResolvedValue({
      kind: "live",
      engineLoaded: true,
      running: false,
    });
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
    mocks.syncRevokePeer.mockResolvedValue(true);

    renderPage();

    const removeButton = await screen.findByRole("button", {
      name: /Remove/,
    });
    fireEvent.click(removeButton);

    expect(screen.getByText("Remove this device?")).toBeTruthy();
    expect(mocks.syncRevokePeer).not.toHaveBeenCalled();

    fireEvent.click(
      within(screen.getByRole("dialog")).getByRole("button", {
        name: "Remove",
      }),
    );

    await waitFor(() =>
      expect(mocks.syncRevokePeer).toHaveBeenCalledWith(VALID_FINGERPRINT),
    );
    await waitFor(() => expect(screen.queryByText("Work laptop")).toBeNull());
    await waitFor(() =>
      expect(screen.getByText("No devices paired yet.")).toBeTruthy(),
    );
  });

  describe("QR / link pairing", () => {
    beforeEach(() => {
      mocks.syncStatus.mockResolvedValue({
        kind: "live",
        engineLoaded: true,
        running: false,
      });
      mocks.syncThisDevice.mockResolvedValue(VALID_FINGERPRINT_GROUPED);
      mocks.syncListPeers.mockResolvedValue([]);
    });

    it("opening 'Pair a device' creates an offer and shows the QR and link", async () => {
      const expiresAt = Math.floor(Date.now() / 1000) + 600;
      mocks.pairingCreateOffer.mockResolvedValue({
        link: "notare://pair#abc123",
        qrSvg: "<svg data-testid='qr'></svg>",
        expiresAt,
      });
      mocks.pairingStatus.mockResolvedValue({
        offerActive: true,
        offerExpiresAt: expiresAt,
        pending: [],
      });

      renderPage();

      const pairButton = await screen.findByRole("button", {
        name: /Pair a device/,
      });
      fireEvent.click(pairButton);

      await waitFor(() =>
        expect(mocks.pairingCreateOffer).toHaveBeenCalledTimes(1),
      );
      await waitFor(() =>
        expect(screen.getByText("notare://pair#abc123")).toBeTruthy(),
      );
      expect(screen.getByText(/Expires in \d+:\d{2}/)).toBeTruthy();
    });

    it("renders the QR as a safe img, never injecting the SVG markup into the DOM (regression: QR SVG dangerouslySetInnerHTML)", async () => {
      const expiresAt = Math.floor(Date.now() / 1000) + 600;
      const maliciousSvg =
        '<svg onload="window.__pwned = true"><script>window.__pwned = true;</script></svg>';
      mocks.pairingCreateOffer.mockResolvedValue({
        link: "notare://pair#abc123",
        qrSvg: maliciousSvg,
        expiresAt,
      });
      mocks.pairingStatus.mockResolvedValue({
        offerActive: true,
        offerExpiresAt: expiresAt,
        pending: [],
      });

      renderPage();

      fireEvent.click(
        await screen.findByRole("button", { name: /Pair a device/ }),
      );

      const img = await screen.findByRole("img", { name: /Pairing QR code/ });
      expect(img.getAttribute("src")).toBe(
        `data:image/svg+xml;utf8,${encodeURIComponent(maliciousSvg)}`,
      );
      const dialog = screen.getByRole("dialog");
      expect(document.querySelector("script")).toBeNull();
      expect(dialog.querySelector("script")).toBeNull();
      expect(dialog.querySelector("[onload]")).toBeNull();
      const windowWithPwnedFlag = window as unknown as { __pwned?: boolean }; // test-only global set if the malicious SVG executed
      expect(windowWithPwnedFlag.__pwned).toBeUndefined();
    });

    it("shows an error with a retry button when offer creation fails, instead of spinning forever (regression: createOffer.isError unreachable)", async () => {
      mocks.pairingCreateOffer.mockRejectedValueOnce(new Error("offline"));

      renderPage();

      fireEvent.click(
        await screen.findByRole("button", { name: /Pair a device/ }),
      );

      await waitFor(() => expect(screen.getByText("offline")).toBeTruthy());
      const retryButton = screen.getByRole("button", { name: /Try again/ });

      const expiresAt = Math.floor(Date.now() / 1000) + 600;
      mocks.pairingCreateOffer.mockResolvedValueOnce({
        link: "notare://pair#retry",
        qrSvg: "<svg></svg>",
        expiresAt,
      });
      mocks.pairingStatus.mockResolvedValue({
        offerActive: true,
        offerExpiresAt: expiresAt,
        pending: [],
      });

      fireEvent.click(retryButton);

      await waitFor(() =>
        expect(screen.getByText("notare://pair#retry")).toBeTruthy(),
      );
    });

    it("shows the SAS confirmation once a joiner's proof lands, and confirming adds the peer", async () => {
      const expiresAt = Math.floor(Date.now() / 1000) + 600;
      mocks.pairingCreateOffer.mockResolvedValue({
        link: "notare://pair#abc123",
        qrSvg: "<svg></svg>",
        expiresAt,
      });
      mocks.pairingStatus.mockResolvedValue({
        offerActive: true,
        offerExpiresAt: expiresAt,
        pending: [
          {
            deviceId: OTHER_FINGERPRINT,
            name: "Pixel 8",
            sas: "482913",
            expiresAt: expiresAt + 120,
          },
        ],
      });
      mocks.pairingConfirm.mockResolvedValue(undefined);
      mocks.syncListPeers.mockResolvedValueOnce([]).mockResolvedValueOnce([
        {
          nodeId: OTHER_FINGERPRINT,
          fingerprint: OTHER_FINGERPRINT,
          label: "Pixel 8",
          addedAt: 1000,
          lastSeen: 0,
        },
      ]);

      renderPage();

      fireEvent.click(
        await screen.findByRole("button", { name: /Pair a device/ }),
      );

      await waitFor(() => expect(screen.getByText("482913")).toBeTruthy());
      expect(screen.getByText(/Pixel 8/)).toBeTruthy();

      fireEvent.click(screen.getByRole("button", { name: /Confirm/ }));

      await waitFor(() =>
        expect(mocks.pairingConfirm).toHaveBeenCalledWith(
          OTHER_FINGERPRINT,
          true,
        ),
      );
      await waitFor(() => expect(screen.queryByText("482913")).toBeNull());
    });

    it("rejecting the SAS confirmation does not add the peer", async () => {
      const expiresAt = Math.floor(Date.now() / 1000) + 600;
      mocks.pairingCreateOffer.mockResolvedValue({
        link: "notare://pair#abc123",
        qrSvg: "<svg></svg>",
        expiresAt,
      });
      mocks.pairingStatus.mockResolvedValue({
        offerActive: true,
        offerExpiresAt: expiresAt,
        pending: [
          {
            deviceId: OTHER_FINGERPRINT,
            name: "Unknown",
            sas: "999999",
            expiresAt: expiresAt + 120,
          },
        ],
      });
      mocks.pairingConfirm.mockResolvedValue(undefined);

      renderPage();

      fireEvent.click(
        await screen.findByRole("button", { name: /Pair a device/ }),
      );

      await waitFor(() => expect(screen.getByText("999999")).toBeTruthy());

      fireEvent.click(screen.getByRole("button", { name: /Reject/ }));

      await waitFor(() =>
        expect(mocks.pairingConfirm).toHaveBeenCalledWith(
          OTHER_FINGERPRINT,
          false,
        ),
      );
      expect(mocks.syncAddPeer).not.toHaveBeenCalled();
    });

    it("joining via a pasted link shows the SAS and, once confirmed, adds the offerer as a peer", async () => {
      mocks.pairingJoin.mockResolvedValue({
        sas: "135790",
        deviceId: OTHER_FINGERPRINT,
        deviceName: "Work laptop",
      });
      mocks.syncAddPeer.mockResolvedValue(OTHER_FINGERPRINT);

      renderPage();

      const linkInput = await screen.findByLabelText("Pairing link");
      fireEvent.change(linkInput, {
        target: { value: "notare://pair#xyz" },
      });
      fireEvent.click(screen.getByRole("button", { name: /Join/ }));

      await waitFor(() =>
        expect(mocks.pairingJoin).toHaveBeenCalledWith(
          "notare://pair#xyz",
          expect.anything(),
        ),
      );
      await waitFor(() => expect(screen.getByText("135790")).toBeTruthy());

      fireEvent.click(screen.getByRole("button", { name: /Confirm/ }));

      await waitFor(() =>
        expect(mocks.syncAddPeer).toHaveBeenCalledWith(
          OTHER_FINGERPRINT,
          "Work laptop",
        ),
      );
    });

    it("rejecting a join's SAS does not add the peer", async () => {
      mocks.pairingJoin.mockResolvedValue({
        sas: "246810",
        deviceId: OTHER_FINGERPRINT,
        deviceName: "Unknown device",
      });

      renderPage();

      const linkInput = await screen.findByLabelText("Pairing link");
      fireEvent.change(linkInput, {
        target: { value: "notare://pair#xyz" },
      });
      fireEvent.click(screen.getByRole("button", { name: /Join/ }));

      await waitFor(() => expect(screen.getByText("246810")).toBeTruthy());

      fireEvent.click(screen.getByRole("button", { name: /Reject/ }));

      expect(mocks.syncAddPeer).not.toHaveBeenCalled();
    });
  });

  describe("sync_enabled runtime gate", () => {
    it("defaults to off: the toggle starts unchecked, no start/stop call is made, and the pairing UI is inert", async () => {
      mocks.syncEnabled = undefined;
      mocks.syncStatus.mockResolvedValue({
        kind: "live",
        engineLoaded: true,
        running: false,
      });
      mocks.syncThisDevice.mockResolvedValue(VALID_FINGERPRINT_GROUPED);
      mocks.syncListPeers.mockResolvedValue([]);

      renderPage();

      const toggle = await screen.findByRole("switch", {
        name: ENABLE_SWITCH_NAME,
      });
      expect(toggle.getAttribute("aria-checked")).toBe("false");
      expect(mocks.syncStart).not.toHaveBeenCalled();
      expect(mocks.syncStop).not.toHaveBeenCalled();

      expect(
        (
          screen.getByRole("button", {
            name: /Pair a device/,
          }) as HTMLButtonElement
        ).disabled,
      ).toBe(true);
      expect(
        (screen.getByLabelText("Pairing link") as HTMLInputElement).disabled,
      ).toBe(true);

      await openAdvanced();
      expect(
        (screen.getByLabelText("Peer fingerprint") as HTMLInputElement)
          .disabled,
      ).toBe(true);
      expect(
        (screen.getByLabelText("Device label") as HTMLInputElement).disabled,
      ).toBe(true);
      expect(
        (
          screen.getByRole("button", {
            name: /Add device/,
          }) as HTMLButtonElement
        ).disabled,
      ).toBe(true);
    });

    it("turning sync on persists the setting and starts the agent without requiring a restart", async () => {
      mocks.syncEnabled = false;
      mocks.syncStatus.mockResolvedValue({
        kind: "live",
        engineLoaded: true,
        running: false,
      });
      mocks.syncThisDevice.mockResolvedValue(VALID_FINGERPRINT_GROUPED);
      mocks.syncListPeers.mockResolvedValue([]);

      renderPage();

      const toggle = await screen.findByRole("switch", {
        name: ENABLE_SWITCH_NAME,
      });
      fireEvent.click(toggle);
      await confirmFirstEnable();

      expect(mocks.setSyncEnabled).toHaveBeenCalledWith("sync_enabled", true);
      await waitFor(() => expect(mocks.syncStart).toHaveBeenCalledTimes(1));
      expect(mocks.syncStop).not.toHaveBeenCalled();

      // Once the setting flips on, the pairing UI stops being inert — no
      // app restart needed.
      await waitFor(() =>
        expect(
          (
            screen.getByRole("button", {
              name: /Pair a device/,
            }) as HTMLButtonElement
          ).disabled,
        ).toBe(false),
      );
    });

    it("turning sync off persists the setting and stops the agent without requiring a restart", async () => {
      mocks.syncEnabled = true;
      mocks.syncStatus.mockResolvedValue({
        kind: "live",
        engineLoaded: true,
        running: true,
      });
      mocks.syncThisDevice.mockResolvedValue(VALID_FINGERPRINT_GROUPED);
      mocks.syncListPeers.mockResolvedValue([]);

      renderPage();

      const toggle = await screen.findByRole("switch", {
        name: ENABLE_SWITCH_NAME,
      });
      fireEvent.click(toggle);

      expect(mocks.setSyncEnabled).toHaveBeenCalledWith("sync_enabled", false);
      await waitFor(() => expect(mocks.syncStop).toHaveBeenCalledTimes(1));
      expect(mocks.syncStart).not.toHaveBeenCalled();

      await waitFor(() =>
        expect(
          (
            screen.getByRole("button", {
              name: /Pair a device/,
            }) as HTMLButtonElement
          ).disabled,
        ).toBe(true),
      );
    });

    it("reverts the setting and re-disables the pairing UI if the agent fails to start", async () => {
      mocks.syncEnabled = false;
      mocks.syncStatus.mockResolvedValue({
        kind: "live",
        engineLoaded: true,
        running: false,
      });
      mocks.syncThisDevice.mockResolvedValue(VALID_FINGERPRINT_GROUPED);
      mocks.syncListPeers.mockResolvedValue([]);
      mocks.syncStart.mockRejectedValue(new Error("agent failed to start"));

      renderPage();

      const toggle = await screen.findByRole("switch", {
        name: ENABLE_SWITCH_NAME,
      });
      fireEvent.click(toggle);
      await confirmFirstEnable();

      // Optimistic: the setting flips on immediately...
      expect(mocks.setSyncEnabled).toHaveBeenNthCalledWith(
        1,
        "sync_enabled",
        true,
      );
      // ...then reverts once syncStart rejects, so the persisted setting and
      // the pairing UI it gates track what the agent actually did.
      await waitFor(() =>
        expect(mocks.setSyncEnabled).toHaveBeenNthCalledWith(
          2,
          "sync_enabled",
          false,
        ),
      );
      await waitFor(() =>
        expect(toggle.getAttribute("aria-checked")).toBe("false"),
      );
      await waitFor(() =>
        expect(
          (
            screen.getByRole("button", {
              name: /Pair a device/,
            }) as HTMLButtonElement
          ).disabled,
        ).toBe(true),
      );
      expect(
        screen.getByText(/Couldn't start sync, so it's been turned back off/),
      ).toBeTruthy();

      // The status line must refetch on failure too, not just success —
      // otherwise it can keep showing a stale pre-attempt read.
      await waitFor(() => expect(mocks.syncStatus).toHaveBeenCalledTimes(2));
    });
  });

  describe("first enable backup prompt", () => {
    beforeEach(() => {
      mocks.syncEnabled = false;
      mocks.syncStatus.mockResolvedValue({
        kind: "live",
        engineLoaded: true,
        running: false,
      });
      mocks.syncThisDevice.mockResolvedValue(VALID_FINGERPRINT_GROUPED);
      mocks.syncListPeers.mockResolvedValue([]);
    });

    it("converts nothing and persists nothing until the user continues", async () => {
      renderPage();

      const toggle = await screen.findByRole("switch", {
        name: ENABLE_SWITCH_NAME,
      });
      fireEvent.click(toggle);

      await screen.findByText(BACKUP_PROMPT);
      expect(mocks.syncStart).not.toHaveBeenCalled();
      expect(mocks.setSyncEnabled).not.toHaveBeenCalled();
      expect(toggle.getAttribute("aria-checked")).toBe("false");

      fireEvent.click(screen.getByRole("button", { name: "Continue" }));

      expect(mocks.setSyncEnabled).toHaveBeenCalledWith("sync_enabled", true);
      await waitFor(() => expect(mocks.syncStart).toHaveBeenCalledTimes(1));
      await waitFor(() => expect(screen.queryByText(BACKUP_PROMPT)).toBeNull());
    });

    it("closing the prompt leaves sync off", async () => {
      renderPage();

      const toggle = await screen.findByRole("switch", {
        name: ENABLE_SWITCH_NAME,
      });
      fireEvent.click(toggle);
      await screen.findByText(BACKUP_PROMPT);

      fireEvent.click(screen.getByRole("button", { name: "Close" }));

      await waitFor(() => expect(screen.queryByText(BACKUP_PROMPT)).toBeNull());
      expect(mocks.syncStart).not.toHaveBeenCalled();
      expect(mocks.setSyncEnabled).not.toHaveBeenCalled();
      expect(toggle.getAttribute("aria-checked")).toBe("false");
    });

    it("opens the app data folder without starting sync", async () => {
      mocks.globalBase.mockResolvedValue({
        status: "ok",
        data: "/home/me/.local/share/notare",
      });
      mocks.openPath.mockResolvedValue({ status: "ok", data: null });

      renderPage();

      fireEvent.click(
        await screen.findByRole("switch", { name: ENABLE_SWITCH_NAME }),
      );
      await screen.findByText(BACKUP_PROMPT);
      fireEvent.click(screen.getByRole("button", { name: /Open data folder/ }));

      await waitFor(() =>
        expect(mocks.openPath).toHaveBeenCalledWith(
          "/home/me/.local/share/notare",
          null,
        ),
      );
      expect(screen.getByText(BACKUP_PROMPT)).toBeTruthy();
      expect(mocks.syncStart).not.toHaveBeenCalled();
      expect(mocks.setSyncEnabled).not.toHaveBeenCalled();
    });

    it("shows why the data folder could not be opened, and forgets it once closed", async () => {
      mocks.globalBase.mockResolvedValue({
        status: "error",
        error: "data dir unavailable",
      });

      renderPage();

      fireEvent.click(
        await screen.findByRole("switch", { name: ENABLE_SWITCH_NAME }),
      );
      await screen.findByText(BACKUP_PROMPT);
      fireEvent.click(screen.getByRole("button", { name: /Open data folder/ }));

      await screen.findByText("data dir unavailable");
      expect(mocks.openPath).not.toHaveBeenCalled();
      expect(mocks.syncStart).not.toHaveBeenCalled();

      fireEvent.click(screen.getByRole("button", { name: "Close" }));
      await waitFor(() => expect(screen.queryByText(BACKUP_PROMPT)).toBeNull());
      fireEvent.click(screen.getByRole("switch", { name: ENABLE_SWITCH_NAME }));
      await screen.findByText(BACKUP_PROMPT);
      expect(screen.queryByText("data dir unavailable")).toBeNull();
    });

    it("asks again if the conversion check fails", async () => {
      mocks.execute.mockRejectedValue(new Error("db busy"));

      renderPage();
      await waitFor(() => expect(mocks.execute).toHaveBeenCalled());

      fireEvent.click(
        await screen.findByRole("switch", { name: ENABLE_SWITCH_NAME }),
      );

      await screen.findByText(BACKUP_PROMPT);
      expect(mocks.syncStart).not.toHaveBeenCalled();
    });

    it("skips the prompt once the synced tables are already converted", async () => {
      mocks.execute.mockResolvedValue([{ n: 6 }]);

      renderPage();
      const toggle = await screen.findByRole("switch", {
        name: ENABLE_SWITCH_NAME,
      });
      await waitFor(() => expect(mocks.execute).toHaveBeenCalled());
      // Let the conversion query settle before toggling.
      await act(() => new Promise((resolve) => setTimeout(resolve, 0)));

      fireEvent.click(toggle);

      await waitFor(() => expect(mocks.syncStart).toHaveBeenCalledTimes(1));
      expect(mocks.setSyncEnabled).toHaveBeenCalledWith("sync_enabled", true);
      expect(screen.queryByText(BACKUP_PROMPT)).toBeNull();
    });
  });

  describe("pairing link opened from the OS", () => {
    const LIVE_STATUS = {
      kind: "live",
      engineLoaded: true,
      running: false,
    };

    beforeEach(() => {
      usePendingPairLink.getState().clear();
      mocks.syncThisDevice.mockResolvedValue(VALID_FINGERPRINT_GROUPED);
      mocks.syncListPeers.mockResolvedValue([]);
    });

    afterEach(() => {
      usePendingPairLink.getState().clear();
    });

    it("pre-fills the link and starts the join, ending at the confirmation numbers", async () => {
      mocks.syncStatus.mockResolvedValue(LIVE_STATUS);
      mocks.pairingJoin.mockResolvedValue({
        sas: "112233",
        deviceId: OTHER_FINGERPRINT,
        deviceName: "Phone",
      });
      usePendingPairLink.getState().set("notare://pair#opened");

      renderPage();

      await waitFor(() =>
        expect(mocks.pairingJoin).toHaveBeenCalledWith(
          "notare://pair#opened",
          expect.anything(),
        ),
      );
      await waitFor(() => expect(screen.getByText("112233")).toBeTruthy());
      expect(
        (screen.getByLabelText("Pairing link") as HTMLInputElement).value,
      ).toBe("notare://pair#opened");
      // Consumed: it must not start a second join on the next render.
      expect(usePendingPairLink.getState().link).toBeNull();
      expect(mocks.pairingJoin).toHaveBeenCalledTimes(1);
    });

    it("starts the join when the link arrives while the page is already open", async () => {
      mocks.syncStatus.mockResolvedValue(LIVE_STATUS);
      mocks.pairingJoin.mockResolvedValue({
        sas: "445566",
        deviceId: OTHER_FINGERPRINT,
        deviceName: "Phone",
      });

      renderPage();
      await screen.findByLabelText("Pairing link");
      expect(mocks.pairingJoin).not.toHaveBeenCalled();

      act(() => usePendingPairLink.getState().set("notare://pair#warm"));

      await waitFor(() =>
        expect(mocks.pairingJoin).toHaveBeenCalledWith(
          "notare://pair#warm",
          expect.anything(),
        ),
      );
      await waitFor(() => expect(screen.getByText("445566")).toBeTruthy());
    });

    it("explains that sync is off, holds the link, and joins once sync is turned on", async () => {
      mocks.syncEnabled = false;
      mocks.syncStatus.mockResolvedValue(LIVE_STATUS);
      mocks.pairingJoin.mockResolvedValue({
        sas: "778899",
        deviceId: OTHER_FINGERPRINT,
        deviceName: "Phone",
      });
      usePendingPairLink.getState().set("notare://pair#held");

      renderPage();

      await waitFor(() =>
        expect(
          screen.getByText(/pairing link was opened, but sync is off/i),
        ).toBeTruthy(),
      );
      expect(mocks.pairingJoin).not.toHaveBeenCalled();

      fireEvent.click(
        await screen.findByRole("switch", {
          name: ENABLE_SWITCH_NAME,
        }),
      );
      await confirmFirstEnable();

      await waitFor(() =>
        expect(mocks.pairingJoin).toHaveBeenCalledWith(
          "notare://pair#held",
          expect.anything(),
        ),
      );
      await waitFor(() => expect(screen.getByText("778899")).toBeTruthy());
      expect(
        screen.queryByText(/pairing link was opened, but sync is off/i),
      ).toBeNull();
    });

    it("says so when this build has no device sync, and drops the link", async () => {
      mocks.syncStatus.mockResolvedValue({ kind: "unavailable" });
      usePendingPairLink.getState().set("notare://pair#nope");

      renderPage();

      await waitFor(() =>
        expect(
          screen.getByText(/pairing link was opened, but device sync isn't/i),
        ).toBeTruthy(),
      );
      expect(usePendingPairLink.getState().link).toBeNull();
      expect(mocks.pairingJoin).not.toHaveBeenCalled();
    });
  });
});

import { cleanup, render, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

/**
 * #31: the persistent dictation orb host used to force `enabled = false` on
 * macOS (`!isMacos && dictation_enabled`). These tests pin down that the
 * orb lifecycle now runs on macOS exactly like it does on Windows/Linux -
 * whenever `dictation_enabled` is true, regardless of platform.
 */

const mocks = vi.hoisted(() => ({
  platform: "macos" as string,
  settings: {
    current: {
      dictation_enabled: true,
      dictation_shortcut: "ctrl+alt+space",
      dictation_output_mode: "batch",
      dictation_paste_at_cursor: true,
      dictation_cleanup: "none",
    } as Record<string, unknown>,
  },
  showOrb: vi.fn(async () => ({ status: "ok" as const, data: null })),
  hideOrb: vi.fn(async () => ({ status: "ok" as const, data: null })),
  startDictation: vi.fn(async () => ({ status: "ok" as const, data: null })),
  stopDictation: vi.fn(async () => ({ status: "ok" as const, data: null })),
  registerGlobalHotkey: vi.fn(async () => ({
    status: "ok" as const,
    data: null,
  })),
  unregisterGlobalHotkey: vi.fn(async () => ({
    status: "ok" as const,
    data: null,
  })),
  listen: vi.fn(async () => vi.fn()),
  orbClickListeners: [] as Array<() => void>,
  hotkeyListeners: [] as Array<() => void>,
  sttConnection: {
    conn: null as null | { provider: string; model: string; baseUrl: string },
    isLocalModel: false,
  },
  sonnerInfo: vi.fn(),
  setSettingValues: vi.fn(),
}));

vi.mock("@tauri-apps/plugin-os", () => ({
  platform: () => mocks.platform,
}));

vi.mock("@hypr/plugin-dictation", () => ({
  commands: {
    showOrb: mocks.showOrb,
    hideOrb: mocks.hideOrb,
    startDictation: mocks.startDictation,
    stopDictation: mocks.stopDictation,
    cleanText: vi.fn(async () => ({ status: "ok", data: "" })),
    deliverText: vi.fn(async () => ({ status: "ok", data: null })),
  },
  events: {
    dictationStateEvent: { listen: mocks.listen, emit: vi.fn(async () => {}) },
    dictationFinishedEvent: { listen: mocks.listen },
    dictationOrbClicked: {
      listen: async (cb: () => void) => {
        mocks.orbClickListeners.push(cb);
        return vi.fn();
      },
    },
  },
}));

vi.mock("@hypr/plugin-shortcut", () => ({
  commands: {
    registerGlobalHotkey: mocks.registerGlobalHotkey,
    unregisterGlobalHotkey: mocks.unregisterGlobalHotkey,
  },
  events: {
    globalHotkeyTriggered: {
      listen: async (cb: () => void) => {
        mocks.hotkeyListeners.push(cb);
        return vi.fn();
      },
    },
  },
}));

vi.mock("~/ai/hooks", () => ({
  useLanguageModel: () => null,
}));

vi.mock("~/settings/queries", () => ({
  useSetSettingValues: () => mocks.setSettingValues,
}));

vi.mock("~/shared/config", () => ({
  useConfigValues: () => mocks.settings.current,
}));

vi.mock("~/stt/useSTTConnection", () => ({
  useSTTConnection: () => mocks.sttConnection,
}));

vi.mock("@hypr/ui/components/ui/toast", () => ({
  sonnerToast: { info: mocks.sonnerInfo },
}));

vi.mock("./history", () => ({
  addDictationHistoryEntry: vi.fn(async () => undefined),
}));

import { DictationOrbHost } from "./host";

describe("DictationOrbHost", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.settings.current = {
      dictation_enabled: true,
      dictation_shortcut: "ctrl+alt+space",
      dictation_output_mode: "batch",
      dictation_paste_at_cursor: true,
      dictation_cleanup: "none",
    };
    mocks.orbClickListeners = [];
    mocks.hotkeyListeners = [];
    mocks.sttConnection = { conn: null, isLocalModel: false };
  });

  afterEach(() => {
    cleanup();
  });

  it("shows the orb and registers the hotkey on macOS when dictation is enabled", async () => {
    mocks.platform = "macos";

    render(<DictationOrbHost />);

    await waitFor(() => expect(mocks.showOrb).toHaveBeenCalledTimes(1));
    await waitFor(() =>
      expect(mocks.registerGlobalHotkey).toHaveBeenCalledWith("ctrl+alt+space"),
    );
  });

  it("stays inert on macOS when the orb setting is off", async () => {
    mocks.platform = "macos";
    mocks.settings.current.dictation_enabled = false;

    render(<DictationOrbHost />);

    // Give any stray effect a tick to fire before asserting it never did.
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(mocks.showOrb).not.toHaveBeenCalled();
    expect(mocks.registerGlobalHotkey).not.toHaveBeenCalled();
  });

  it("shows the orb on Windows/Linux too (unchanged behavior)", async () => {
    mocks.platform = "windows";

    render(<DictationOrbHost />);

    await waitFor(() => expect(mocks.showOrb).toHaveBeenCalledTimes(1));
  });

  it("surfaces a toast instead of a silent no-op when no local model is configured", async () => {
    mocks.platform = "macos";
    mocks.sttConnection = { conn: null, isLocalModel: false };

    render(<DictationOrbHost />);
    await waitFor(() => expect(mocks.showOrb).toHaveBeenCalledTimes(1));
    await waitFor(() => expect(mocks.orbClickListeners.length).toBe(1));

    // Simulate an orb click with no local live model ready.
    mocks.orbClickListeners[0]!();

    expect(mocks.startDictation).not.toHaveBeenCalled();
    expect(mocks.sonnerInfo).toHaveBeenCalledTimes(1);
    expect(mocks.sonnerInfo.mock.calls[0]![0]).toMatch(
      /downloaded local model/i,
    );
  });

  it("starts dictation from the orb click when a local model is ready", async () => {
    mocks.platform = "macos";
    mocks.sttConnection = {
      conn: {
        provider: "hyprnote",
        model: "QuantizedTiny",
        baseUrl: "http://127.0.0.1:5555",
      },
      isLocalModel: true,
    };

    render(<DictationOrbHost />);
    await waitFor(() => expect(mocks.orbClickListeners.length).toBe(1));

    mocks.orbClickListeners[0]!();

    await waitFor(() =>
      expect(mocks.startDictation).toHaveBeenCalledWith(
        "http://127.0.0.1:5555",
        "QuantizedTiny",
        "batch",
      ),
    );
    expect(mocks.sonnerInfo).not.toHaveBeenCalled();
  });
});

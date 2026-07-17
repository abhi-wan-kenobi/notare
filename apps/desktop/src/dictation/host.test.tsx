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
  setSettingValues: vi.fn(),
}));

vi.mock("@tauri-apps/plugin-os", () => ({
  platform: () => mocks.platform,
}));

vi.mock("@hypr/plugin-dictation", () => ({
  commands: {
    showOrb: mocks.showOrb,
    hideOrb: mocks.hideOrb,
    startDictation: vi.fn(async () => ({ status: "ok", data: null })),
    stopDictation: mocks.stopDictation,
    cleanText: vi.fn(async () => ({ status: "ok", data: "" })),
    deliverText: vi.fn(async () => ({ status: "ok", data: null })),
  },
  events: {
    dictationStateEvent: { listen: mocks.listen, emit: vi.fn(async () => {}) },
    dictationFinishedEvent: { listen: mocks.listen },
    dictationOrbClicked: { listen: mocks.listen },
  },
}));

vi.mock("@hypr/plugin-shortcut", () => ({
  commands: {
    registerGlobalHotkey: mocks.registerGlobalHotkey,
    unregisterGlobalHotkey: mocks.unregisterGlobalHotkey,
  },
  events: {
    globalHotkeyTriggered: { listen: mocks.listen },
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
  useSTTConnection: () => ({ conn: null, isLocalModel: false }),
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
  });

  afterEach(() => {
    cleanup();
  });

  it("shows the orb and registers the hotkey on macOS when dictation is enabled", async () => {
    mocks.platform = "macos";

    render(<DictationOrbHost />);

    await waitFor(() => expect(mocks.showOrb).toHaveBeenCalledTimes(1));
    await waitFor(() =>
      expect(mocks.registerGlobalHotkey).toHaveBeenCalledWith(
        "ctrl+alt+space",
      ),
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
});

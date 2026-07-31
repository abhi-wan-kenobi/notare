import { cleanup, render, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

/**
 * #31: the persistent dictation orb host used to force `enabled = false` on
 * macOS (`!isMacos && dictation_enabled`). These tests pin down that the
 * orb lifecycle now runs on macOS exactly like it does on Windows/Linux -
 * whenever `dictation_enabled` is true, regardless of platform.
 */

type CmdResult =
  | { status: "ok"; data: null }
  | { status: "error"; error: string };

const mocks = vi.hoisted(() => ({
  platform: "macos" as string,
  settings: {
    current: {
      dictation_enabled: true,
      dictation_shortcut: "ctrl+alt+space",
      dictation_paste_last_shortcut: "",
      dictation_output_mode: "batch",
      dictation_paste_at_cursor: true,
      dictation_cleanup: "none",
    } as Record<string, unknown>,
  },
  showOrb: vi.fn(async () => ({ status: "ok" as const, data: null })),
  hideOrb: vi.fn(async () => ({ status: "ok" as const, data: null })),
  startDictation: vi.fn<() => Promise<CmdResult>>(async () => ({
    status: "ok",
    data: null,
  })),
  stopDictation: vi.fn(async () => ({ status: "ok" as const, data: null })),
  isDictating: vi.fn(async () => ({ status: "ok" as const, data: false })),
  registerGlobalHotkey: vi.fn(async () => ({
    status: "ok" as const,
    data: null,
  })),
  unregisterGlobalHotkey: vi.fn(async () => ({
    status: "ok" as const,
    data: null,
  })),
  deliverText: vi.fn<() => Promise<CmdResult>>(async () => ({
    status: "ok",
    data: null,
  })),
  listDictationHistory: vi.fn(async () => ({
    entries: mocks.historyEntries,
    nextCursor: null as string | null,
  })),
  historyEntries: [] as Array<{
    id: string;
    text: string;
    status: "delivered" | "discarded";
  }>,
  listen: vi.fn(async () => vi.fn()),
  orbClickListeners: [] as Array<() => void>,
  hotkeyListeners: [] as Array<
    (event: { payload: { id: string; shortcut: string } }) => void
  >,
  stateListeners: [] as Array<(event: { payload: unknown }) => void>,
  hideRequestListeners: [] as Array<() => void>,
  listenerState: { live: { status: "inactive" } } as {
    live: { status: string };
  },
  listenerSubscribers: [] as Array<
    (state: { live: { status: string } }) => void
  >,
  sttConnection: {
    conn: null as null | { provider: string; model: string; baseUrl: string },
    isLocalModel: false,
  },
  sonnerInfo: vi.fn(),
  sonnerError: vi.fn(),
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
    isDictating: mocks.isDictating,
    cleanText: vi.fn(async () => ({ status: "ok", data: "" })),
    deliverText: mocks.deliverText,
  },
  events: {
    dictationStateEvent: {
      listen: async (cb: (event: { payload: unknown }) => void) => {
        mocks.stateListeners.push(cb);
        return vi.fn();
      },
      emit: vi.fn(async () => {}),
    },
    dictationFinishedEvent: { listen: mocks.listen },
    dictationOrbClicked: {
      listen: async (cb: () => void) => {
        mocks.orbClickListeners.push(cb);
        return vi.fn();
      },
    },
    dictationOrbHideRequested: {
      listen: async (cb: () => void) => {
        mocks.hideRequestListeners.push(cb);
        return vi.fn();
      },
    },
  },
}));

vi.mock("~/store/zustand/listener/instance", () => ({
  listenerStore: {
    getState: () => mocks.listenerState,
    subscribe: (cb: (state: { live: { status: string } }) => void) => {
      mocks.listenerSubscribers.push(cb);
      return () => {
        const index = mocks.listenerSubscribers.indexOf(cb);
        if (index >= 0) {
          mocks.listenerSubscribers.splice(index, 1);
        }
      };
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
      listen: async (
        cb: (event: { payload: { id: string; shortcut: string } }) => void,
      ) => {
        mocks.hotkeyListeners.push(cb);
        return vi.fn();
      },
    },
  },
}));

vi.mock("~/ai/hooks", () => ({
  useLanguageModel: () => null,
  useScopedLanguageModel: () => null,
}));

vi.mock("~/settings/queries", () => ({
  useSetSettingValues: () => mocks.setSettingValues,
  useStoredSettingValue: () => ({ value: undefined, hasValue: false }),
}));

vi.mock("~/shared/config", () => ({
  useConfigValues: () => mocks.settings.current,
}));

vi.mock("~/stt/useSTTConnection", () => ({
  useSTTConnection: () => mocks.sttConnection,
}));

vi.mock("@hypr/ui/components/ui/toast", () => ({
  sonnerToast: { info: mocks.sonnerInfo, error: mocks.sonnerError },
}));

vi.mock("./history", () => ({
  addDictationHistoryEntry: vi.fn(async () => undefined),
  listDictationHistory: mocks.listDictationHistory,
}));

import { DictationOrbHost } from "./host";

function setMeetingActive(active: boolean) {
  mocks.listenerState = { live: { status: active ? "active" : "inactive" } };
  for (const cb of mocks.listenerSubscribers) {
    cb(mocks.listenerState);
  }
}

function pushPhase(phase: string) {
  for (const cb of mocks.stateListeners) {
    cb({ payload: { phase, amplitude: 0, mode: "type" } });
  }
}

function triggerHotkey(id: string, shortcut: string) {
  for (const cb of mocks.hotkeyListeners) {
    cb({ payload: { id, shortcut } });
  }
}

describe("DictationOrbHost", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.settings.current = {
      dictation_enabled: true,
      dictation_shortcut: "ctrl+alt+space",
      dictation_paste_last_shortcut: "",
      dictation_output_mode: "batch",
      dictation_paste_at_cursor: true,
      dictation_cleanup: "none",
    };
    mocks.historyEntries = [];
    mocks.deliverText.mockResolvedValue({ status: "ok", data: null });
    mocks.orbClickListeners = [];
    mocks.hotkeyListeners = [];
    mocks.stateListeners = [];
    mocks.hideRequestListeners = [];
    mocks.listenerState = { live: { status: "inactive" } };
    mocks.listenerSubscribers = [];
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
      expect(mocks.registerGlobalHotkey).toHaveBeenCalledWith(
        "dictation_toggle",
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

  // Regression coverage for the macOS "orb click does nothing, silently" bug:
  // a rejected/errored `startDictation` (e.g. the Soniqo live bridge failing
  // to start) must surface, never disappear into a `void`ed promise.
  it("surfaces a toast when startDictation resolves with an error", async () => {
    mocks.platform = "macos";
    mocks.sttConnection = {
      conn: {
        provider: "hyprnote",
        model: "soniqo-parakeet-streaming",
        baseUrl: "soniqo://local",
      },
      isLocalModel: true,
    };
    mocks.startDictation.mockResolvedValueOnce({
      status: "error" as const,
      error: "soniqo_live_start_failed: model not ready",
    });

    render(<DictationOrbHost />);
    await waitFor(() => expect(mocks.orbClickListeners.length).toBe(1));

    mocks.orbClickListeners[0]!();

    await waitFor(() => expect(mocks.sonnerError).toHaveBeenCalledTimes(1));
    expect(mocks.sonnerError.mock.calls[0]![0]).toMatch(
      /couldn't start dictation/i,
    );
  });

  it("hides the orb while a meeting recording is live and re-shows it after", async () => {
    render(<DictationOrbHost />);
    await waitFor(() => expect(mocks.showOrb).toHaveBeenCalledTimes(1));
    await waitFor(() => expect(mocks.listenerSubscribers.length).toBe(1));

    setMeetingActive(true);
    await waitFor(() => expect(mocks.hideOrb).toHaveBeenCalledTimes(1));

    setMeetingActive(false);
    await waitFor(() => expect(mocks.showOrb).toHaveBeenCalledTimes(2));
  });

  it("never shows the orb when a meeting recording is already live on mount", async () => {
    mocks.listenerState = { live: { status: "active" } };

    render(<DictationOrbHost />);

    await waitFor(() => expect(mocks.hideOrb).toHaveBeenCalledTimes(1));
    expect(mocks.showOrb).not.toHaveBeenCalled();
  });

  it("hides the orb on a right-click dismissal until the next dictation start", async () => {
    render(<DictationOrbHost />);
    await waitFor(() => expect(mocks.showOrb).toHaveBeenCalledTimes(1));
    await waitFor(() => expect(mocks.hideRequestListeners.length).toBe(1));

    mocks.hideRequestListeners[0]!();
    await waitFor(() => expect(mocks.hideOrb).toHaveBeenCalledTimes(1));

    // The next session start (phase -> listening) re-arms visibility.
    pushPhase("listening");
    await waitFor(() => expect(mocks.showOrb).toHaveBeenCalledTimes(2));
  });

  // Mid-meeting dictation still needs its mic indicator: starting a session
  // overrides the meeting suppression, and ending it re-applies it.
  // The orb window's right-click guard reads an async copy of the phase, so
  // a stale hide request can slip out just as a session starts - the host
  // must drop it rather than hide the indicator of a live mic.
  it("ignores a hide request that races a session start", async () => {
    render(<DictationOrbHost />);
    await waitFor(() => expect(mocks.showOrb).toHaveBeenCalledTimes(1));
    await waitFor(() => expect(mocks.hideRequestListeners.length).toBe(1));

    pushPhase("listening");
    mocks.hideRequestListeners[0]!();

    // Give the (wrongly) queued hide a tick to fire before asserting.
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(mocks.hideOrb).not.toHaveBeenCalled();
  });

  // Mounting with a session already live (a failed async stop from the
  // previous effect run) must seed `dictating` from the Rust side - a live
  // mic must keep its indicator even if a meeting is recording.
  it("seeds the dictating state from isDictating on mount", async () => {
    mocks.listenerState = { live: { status: "active" } };
    mocks.isDictating.mockResolvedValueOnce({
      status: "ok" as const,
      data: true,
    });

    render(<DictationOrbHost />);

    await waitFor(() => expect(mocks.showOrb).toHaveBeenCalledTimes(1));
  });

  it("shows the orb for a dictation started mid-meeting and re-hides it after", async () => {
    render(<DictationOrbHost />);
    await waitFor(() => expect(mocks.showOrb).toHaveBeenCalledTimes(1));
    await waitFor(() => expect(mocks.listenerSubscribers.length).toBe(1));

    setMeetingActive(true);
    await waitFor(() => expect(mocks.hideOrb).toHaveBeenCalledTimes(1));

    pushPhase("listening");
    await waitFor(() => expect(mocks.showOrb).toHaveBeenCalledTimes(2));

    pushPhase("idle");
    await waitFor(() => expect(mocks.hideOrb).toHaveBeenCalledTimes(2));
  });

  it("surfaces a toast when startDictation rejects", async () => {
    mocks.platform = "macos";
    mocks.sttConnection = {
      conn: {
        provider: "hyprnote",
        model: "soniqo-parakeet-streaming",
        baseUrl: "soniqo://local",
      },
      isLocalModel: true,
    };
    mocks.startDictation.mockRejectedValueOnce(new Error("ipc failure"));

    render(<DictationOrbHost />);
    await waitFor(() => expect(mocks.orbClickListeners.length).toBe(1));

    mocks.orbClickListeners[0]!();

    await waitFor(() => expect(mocks.sonnerError).toHaveBeenCalledTimes(1));
  });

  // Lane A1: the second global hotkey — paste the most recent delivered
  // dictation at the cursor.
  describe("paste-last-dictation hotkey", () => {
    it("registers the paste-last hotkey (keyed) when one is set", async () => {
      mocks.settings.current.dictation_paste_last_shortcut = "ctrl+alt+v";

      render(<DictationOrbHost />);

      await waitFor(() =>
        expect(mocks.registerGlobalHotkey).toHaveBeenCalledWith(
          "dictation_paste_last",
          "ctrl+alt+v",
        ),
      );
      // The toggle stays registered under its own key alongside it.
      expect(mocks.registerGlobalHotkey).toHaveBeenCalledWith(
        "dictation_toggle",
        "ctrl+alt+space",
      );
    });

    it("does not register the paste-last hotkey when unset", async () => {
      mocks.settings.current.dictation_paste_last_shortcut = "";

      render(<DictationOrbHost />);

      await waitFor(() =>
        expect(mocks.registerGlobalHotkey).toHaveBeenCalledWith(
          "dictation_toggle",
          "ctrl+alt+space",
        ),
      );
      expect(mocks.registerGlobalHotkey).not.toHaveBeenCalledWith(
        "dictation_paste_last",
        expect.anything(),
      );
    });

    it("skips registration when it collides with the toggle shortcut", async () => {
      mocks.settings.current.dictation_paste_last_shortcut = "ctrl+alt+space";

      render(<DictationOrbHost />);

      await waitFor(() =>
        expect(mocks.registerGlobalHotkey).toHaveBeenCalledWith(
          "dictation_toggle",
          "ctrl+alt+space",
        ),
      );
      expect(mocks.registerGlobalHotkey).not.toHaveBeenCalledWith(
        "dictation_paste_last",
        expect.anything(),
      );
    });

    it("pastes the newest delivered entry, skipping a discarded newer one", async () => {
      mocks.settings.current.dictation_paste_last_shortcut = "ctrl+alt+v";
      mocks.historyEntries = [
        { id: "3", text: "  ", status: "delivered" }, // newest but empty -> skip
        { id: "2", text: "recovered draft", status: "discarded" }, // skip
        { id: "1", text: "hello world", status: "delivered" },
      ];

      render(<DictationOrbHost />);
      await waitFor(() => expect(mocks.hotkeyListeners.length).toBe(1));

      triggerHotkey("dictation_paste_last", "ctrl+alt+v");

      await waitFor(() =>
        expect(mocks.deliverText).toHaveBeenCalledWith("hello world", true),
      );
      expect(mocks.deliverText).toHaveBeenCalledTimes(1);
      expect(mocks.sonnerInfo).not.toHaveBeenCalled();
    });

    // Pasting mid-session would interleave the previous transcript with the
    // live one in whatever app has focus.
    it("refuses to paste while a dictation session is active", async () => {
      mocks.settings.current.dictation_paste_last_shortcut = "ctrl+alt+v";
      mocks.historyEntries = [{ id: "1", text: "hi", status: "delivered" }];

      render(<DictationOrbHost />);
      await waitFor(() => expect(mocks.hotkeyListeners.length).toBe(1));

      pushPhase("listening");
      triggerHotkey("dictation_paste_last", "ctrl+alt+v");

      await new Promise((resolve) => setTimeout(resolve, 0));
      expect(mocks.deliverText).not.toHaveBeenCalled();
      expect(mocks.sonnerInfo).toHaveBeenCalledTimes(1);
    });

    it("does not paste for the toggle hotkey id (routes toggle -> dictation)", async () => {
      mocks.settings.current.dictation_paste_last_shortcut = "ctrl+alt+v";
      mocks.historyEntries = [{ id: "1", text: "hi", status: "delivered" }];

      render(<DictationOrbHost />);
      await waitFor(() => expect(mocks.hotkeyListeners.length).toBe(1));

      triggerHotkey("dictation_toggle", "ctrl+alt+space");

      // Give any (wrong) async paste a tick to fire before asserting it didn't.
      await new Promise((resolve) => setTimeout(resolve, 0));
      expect(mocks.deliverText).not.toHaveBeenCalled();
    });

    it("shows an info toast and does not deliver when history is empty", async () => {
      mocks.settings.current.dictation_paste_last_shortcut = "ctrl+alt+v";
      mocks.historyEntries = [];

      render(<DictationOrbHost />);
      await waitFor(() => expect(mocks.hotkeyListeners.length).toBe(1));

      triggerHotkey("dictation_paste_last", "ctrl+alt+v");

      await waitFor(() => expect(mocks.sonnerInfo).toHaveBeenCalledTimes(1));
      expect(mocks.sonnerInfo.mock.calls[0]![0]).toMatch(/no dictation/i);
      expect(mocks.deliverText).not.toHaveBeenCalled();
    });

    it("shows an error toast when the paste delivery fails", async () => {
      mocks.settings.current.dictation_paste_last_shortcut = "ctrl+alt+v";
      mocks.historyEntries = [{ id: "1", text: "hello", status: "delivered" }];
      mocks.deliverText.mockResolvedValueOnce({
        status: "error",
        error: "paste_failed",
      });

      render(<DictationOrbHost />);
      await waitFor(() => expect(mocks.hotkeyListeners.length).toBe(1));

      triggerHotkey("dictation_paste_last", "ctrl+alt+v");

      await waitFor(() => expect(mocks.sonnerError).toHaveBeenCalledTimes(1));
      expect(mocks.sonnerError.mock.calls[0]![0]).toMatch(/paste the last/i);
    });

    it("ignores hotkey spam while a paste is already in flight", async () => {
      mocks.settings.current.dictation_paste_last_shortcut = "ctrl+alt+v";
      mocks.historyEntries = [{ id: "1", text: "hello", status: "delivered" }];

      // Hold the first delivery open so the second trigger lands mid-flight.
      let resolveDeliver: (value: CmdResult) => void = () => {};
      mocks.deliverText.mockImplementationOnce(
        () =>
          new Promise<CmdResult>((resolve) => {
            resolveDeliver = resolve;
          }),
      );

      render(<DictationOrbHost />);
      await waitFor(() => expect(mocks.hotkeyListeners.length).toBe(1));

      triggerHotkey("dictation_paste_last", "ctrl+alt+v");
      await waitFor(() => expect(mocks.deliverText).toHaveBeenCalledTimes(1));

      // Second press while the first delivery is still pending: no double-paste.
      triggerHotkey("dictation_paste_last", "ctrl+alt+v");
      await new Promise((resolve) => setTimeout(resolve, 0));
      expect(mocks.deliverText).toHaveBeenCalledTimes(1);

      // Once the in-flight paste settles, a fresh press pastes again.
      resolveDeliver({ status: "ok", data: null });
      await waitFor(() => expect(mocks.deliverText).toHaveBeenCalledTimes(1));

      triggerHotkey("dictation_paste_last", "ctrl+alt+v");
      await waitFor(() => expect(mocks.deliverText).toHaveBeenCalledTimes(2));
    });
  });
});

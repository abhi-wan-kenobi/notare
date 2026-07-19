import { randomUUID } from "node:crypto";
import * as React from "react";
import { vi } from "vitest";

Object.defineProperty(globalThis.crypto, "randomUUID", { value: randomUUID });

// jsdom ships no canvas implementation and logs a noisy "Not implemented"
// error on every getContext() call (e.g. the dictation particle orb, which
// treats a null context as "render nothing"). Return null quietly instead.
HTMLCanvasElement.prototype.getContext = vi.fn(
  () => null,
) as unknown as HTMLCanvasElement["getContext"];

// jsdom's `PointerEvent` extends `Event` (not `MouseEvent`), so it silently
// drops the coordinate/button fields (`clientX`, `clientY`, `button`, …) from
// its init dict. Pointer-driven UI - e.g. the main-area window-drag strip in
// `src/main/body.tsx` - then never sees the coordinates a test fires with, and
// `fireEvent.pointerDown/Move` become no-ops for that logic. Back `PointerEvent`
// with `MouseEvent`, which carries those fields, and layer the pointer-specific
// properties on top.
class PointerEventPolyfill extends MouseEvent {
  public readonly pointerId: number;
  public readonly pointerType: string;
  public readonly width: number;
  public readonly height: number;
  public readonly pressure: number;
  public readonly tangentialPressure: number;
  public readonly tiltX: number;
  public readonly tiltY: number;
  public readonly twist: number;
  public readonly isPrimary: boolean;

  constructor(type: string, params: PointerEventInit = {}) {
    super(type, params);
    this.pointerId = params.pointerId ?? 0;
    this.pointerType = params.pointerType ?? "";
    this.width = params.width ?? 1;
    this.height = params.height ?? 1;
    this.pressure = params.pressure ?? 0;
    this.tangentialPressure = params.tangentialPressure ?? 0;
    this.tiltX = params.tiltX ?? 0;
    this.tiltY = params.tiltY ?? 0;
    this.twist = params.twist ?? 0;
    this.isPrimary = params.isPrimary ?? false;
  }
}
globalThis.PointerEvent =
  PointerEventPolyfill as unknown as typeof globalThis.PointerEvent;
globalThis.window.PointerEvent = globalThis.PointerEvent;

Object.defineProperty(globalThis.window, "__TAURI_INTERNALS__", {
  value: {
    metadata: {
      currentWindow: {
        label: "main",
      },
      currentWebview: {
        label: "main",
      },
    },
    transformCallback: vi.fn((callback: unknown) => {
      const callbackId = Math.trunc(Math.random() * Number.MAX_SAFE_INTEGER);
      Object.assign(globalThis.window, {
        [`_${callbackId}`]: callback,
      });

      return callbackId;
    }),
    unregisterCallback: vi.fn((callbackId: number) => {
      delete (globalThis.window as unknown as Record<string, unknown>)[
        `_${callbackId}`
      ];
    }),
    invoke: vi.fn((command: string) =>
      Promise.resolve(command === "plugin:event|listen" ? 0 : null),
    ),
  },
  writable: true,
  configurable: true,
});

Object.defineProperty(globalThis.window, "__TAURI_EVENT_PLUGIN_INTERNALS__", {
  value: {
    unregisterListener: vi.fn(),
  },
  writable: true,
  configurable: true,
});

vi.mock("@tauri-apps/api/path", () => ({
  resolveResource: vi.fn((path: string) =>
    Promise.resolve(`/resources/${path}`),
  ),
  sep: vi.fn().mockReturnValue("/"),
}));

vi.mock("@hypr/plugin-db", () => ({
  execute: vi.fn().mockResolvedValue([]),
  executeProxy: vi.fn().mockResolvedValue({ rows: [] }),
  executeTransaction: vi.fn().mockResolvedValue([]),
  getMeeting: vi.fn(),
  getMeetingTranscript: vi.fn(),
  getRecurringMeetingHistory: vi.fn(),
  listMeetings: vi.fn(),
  subscribe: vi.fn().mockResolvedValue(() => Promise.resolve()),
}));

function translate(
  input:
    | TemplateStringsArray
    | string
    | { message?: string; values?: Record<string, unknown> },
  ...values: unknown[]
) {
  if (typeof input === "string") {
    return input;
  }

  if (typeof input === "object" && !("raw" in input)) {
    let message = input.message ?? "";
    for (const [key, value] of Object.entries(input.values ?? {})) {
      message = message.split(`{${key}}`).join(String(value));
    }
    return message;
  }

  return Array.from(input).reduce(
    (text, part, index) => `${text}${part}${values[index] ?? ""}`,
    "",
  );
}

vi.mock("@lingui/react/macro", () => ({
  Trans: ({
    children,
    id,
    message,
    values,
  }: {
    children?: React.ReactNode;
    id?: string;
    message?: string;
    values?: Record<string, unknown>;
  }) =>
    React.createElement(
      React.Fragment,
      null,
      children ?? translate({ message: message ?? id, values }),
    ),
  useLingui: () => ({
    _: translate,
    t: translate,
  }),
}));

vi.mock("@lingui/react", () => ({
  I18nProvider: ({ children }: { children?: React.ReactNode }) =>
    React.createElement(React.Fragment, null, children),
  Trans: ({
    children,
    id,
    message,
    values,
  }: {
    children?: React.ReactNode;
    id?: string;
    message?: string;
    values?: Record<string, unknown>;
  }) =>
    React.createElement(
      React.Fragment,
      null,
      children ?? translate({ message: message ?? id, values }),
    ),
  useLingui: () => ({
    _: translate,
    t: translate,
    i18n: { locale: "en" },
  }),
}));

vi.mock("@hypr/plugin-analytics", () => ({
  commands: {
    event: vi.fn().mockResolvedValue({ status: "ok", data: null }),
    setProperties: vi.fn().mockResolvedValue({ status: "ok", data: null }),
    setDisabled: vi.fn().mockResolvedValue({ status: "ok", data: null }),
    isDisabled: vi.fn().mockResolvedValue({ status: "ok", data: false }),
  },
}));

vi.mock("./types/tauri.gen", () => ({
  commands: {
    getOnboardingNeeded: vi
      .fn()
      .mockResolvedValue({ status: "ok", data: false }),
    showDevtool: vi.fn().mockResolvedValue(true),
    getPinnedTabs: vi.fn().mockResolvedValue({ status: "ok", data: null }),
    setPinnedTabs: vi.fn().mockResolvedValue({ status: "ok", data: null }),
    getRecentlyOpenedSessions: vi
      .fn()
      .mockResolvedValue({ status: "ok", data: null }),
    setRecentlyOpenedSessions: vi
      .fn()
      .mockResolvedValue({ status: "ok", data: null }),
  },
}));

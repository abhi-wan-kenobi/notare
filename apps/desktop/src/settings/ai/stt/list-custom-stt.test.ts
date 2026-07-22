import { afterEach, describe, expect, test, vi } from "vitest";

import { listCustomSttModels, parseCustomSttModels } from "./list-custom-stt";

const { fetchMock } = vi.hoisted(() => ({ fetchMock: vi.fn() }));

vi.mock("@tauri-apps/plugin-http", () => ({
  fetch: fetchMock,
}));

afterEach(() => {
  fetchMock.mockReset();
});

describe("parseCustomSttModels", () => {
  // Regression guard for the shipped-in-#50 bug: the server serializes
  // `ModelIntegrity` as a serde-tagged object (`{ state: "verified" }`), not a
  // bare string, so `installed`/`corrupt` must be derived from `integrity.state`.
  // Reading it as a plain string left every model permanently "not installed",
  // which showed a Download icon on installed models and made rows unclickable.
  test("classifies installed/active from the tagged integrity object", () => {
    const raw = {
      models: [
        {
          id: "QuantizedLargeTurbo",
          displayName: "Large v3 Turbo (Q8)",
          sizeBytes: 874000000,
          englishOnly: false,
          active: true,
          integrity: { state: "verified" },
        },
        {
          id: "QuantizedSmall",
          displayName: "Small",
          active: false,
          integrity: { state: "notInstalled" },
        },
        {
          id: "QuantizedMedium",
          displayName: "Medium",
          active: false,
          integrity: { state: "presentUnverified" },
        },
        {
          id: "QuantizedBase",
          displayName: "Base",
          active: false,
          integrity: { state: "corrupt", detail: "checksum mismatch" },
        },
      ],
    };

    const models = parseCustomSttModels(raw);
    expect(models).toHaveLength(4);

    const byId = Object.fromEntries(models.map((m) => [m.id, m]));
    // verified → installed, and honours `active`
    expect(byId.QuantizedLargeTurbo).toMatchObject({
      installed: true,
      corrupt: false,
      active: true,
    });
    // notInstalled → needs download
    expect(byId.QuantizedSmall).toMatchObject({
      installed: false,
      corrupt: false,
      active: false,
    });
    // presentUnverified → still usable, counts as installed
    expect(byId.QuantizedMedium).toMatchObject({
      installed: true,
      corrupt: false,
    });
    // corrupt → not installed, re-downloadable
    expect(byId.QuantizedBase).toMatchObject({
      installed: false,
      corrupt: true,
    });
  });

  test("tolerates a bare-string integrity (defensive fallback)", () => {
    const models = parseCustomSttModels({
      models: [{ id: "X", integrity: "verified", active: false }],
    });
    expect(models[0]).toMatchObject({ installed: true, corrupt: false });
  });

  test("returns [] for a malformed payload", () => {
    expect(parseCustomSttModels(null)).toEqual([]);
    expect(parseCustomSttModels({})).toEqual([]);
    expect(parseCustomSttModels({ models: "nope" })).toEqual([]);
  });

  // PG-1: an absent or `null` integrity is "unknown" — the server didn't tell
  // us the model's state, so we must NOT classify it as confidently
  // not-installed (which would render a misleading Download CTA on a model that
  // may actually be installed). Same for an unrecognized state string.
  test("marks absent/null/unrecognized integrity as unknown (not not-installed)", () => {
    const models = parseCustomSttModels({
      models: [
        { id: "NoKey", active: false },
        { id: "NullIntegrity", active: false, integrity: null },
        { id: "WeirdState", active: false, integrity: { state: "flux" } },
        {
          id: "NotInstalled",
          active: false,
          integrity: { state: "notInstalled" },
        },
      ],
    });

    const byId = Object.fromEntries(models.map((m) => [m.id, m]));
    expect(byId.NoKey).toMatchObject({
      installed: false,
      corrupt: false,
      unknown: true,
    });
    expect(byId.NullIntegrity).toMatchObject({
      installed: false,
      corrupt: false,
      unknown: true,
    });
    expect(byId.WeirdState).toMatchObject({
      installed: false,
      corrupt: false,
      unknown: true,
    });
    // A genuine notInstalled stays a confident not-installed (not unknown), so
    // the Download CTA still applies where the server actually said so.
    expect(byId.NotInstalled).toMatchObject({
      installed: false,
      corrupt: false,
      unknown: false,
    });
  });
});

describe("listCustomSttModels", () => {
  // PG-4: react-query passes an AbortSignal so a stale in-flight request (the
  // URL/key changed mid-flight) gets cancelled and its result ignored. The
  // signal must reach the fetch and abort it.
  test("threads an externally-supplied AbortSignal into the fetch", async () => {
    const controller = new AbortController();
    controller.abort();

    fetchMock.mockImplementation(async (_url, init) => {
      const signal = (init as { signal: AbortSignal }).signal;
      expect(signal.aborted).toBe(true);
      throw new DOMException("aborted", "AbortError");
    });

    const result = await listCustomSttModels(
      "http://192.168.0.91:8383/v1",
      "",
      controller.signal,
    );

    expect(result.ok).toBe(false);
    // An abort from the caller (not our timeout) is reported as cancelled.
    expect(result.ok === false && result.error).toBe("Cancelled.");
    expect(fetchMock).toHaveBeenCalledTimes(1);
  });

  test("treats a self-timeout abort as a timeout, not a cancel", async () => {
    fetchMock.mockImplementation(async () => {
      throw new DOMException("aborted", "AbortError");
    });

    const result = await listCustomSttModels("http://192.168.0.91:8383/v1", "");

    expect(result.ok).toBe(false);
    expect(result.ok === false && result.error).toBe(
      "Timed out waiting for a response.",
    );
  });
});

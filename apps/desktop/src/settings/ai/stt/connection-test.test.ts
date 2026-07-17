import { describe, expect, test, vi } from "vitest";

import {
  fetchSttServerStatus,
  getSttServerStatusUrl,
  parseSttServerStatus,
} from "./connection-test";

const { fetchMock } = vi.hoisted(() => ({ fetchMock: vi.fn() }));

vi.mock("@tauri-apps/plugin-http", () => ({
  fetch: fetchMock,
}));

describe("getSttServerStatusUrl", () => {
  test("derives the status URL from the origin, dropping the /v1 path", () => {
    expect(getSttServerStatusUrl("http://192.168.0.91:8383/v1")).toBe(
      "http://192.168.0.91:8383/api/status",
    );
  });

  test("preserves an explicit https scheme", () => {
    expect(getSttServerStatusUrl("https://coruscant.lan:8383/v1")).toBe(
      "https://coruscant.lan:8383/api/status",
    );
  });

  test("works with a bare origin (no path)", () => {
    expect(getSttServerStatusUrl("http://localhost:8383")).toBe(
      "http://localhost:8383/api/status",
    );
  });

  test("throws for an invalid URL", () => {
    expect(() => getSttServerStatusUrl("not a url")).toThrow();
  });
});

describe("parseSttServerStatus", () => {
  test("parses the real coruscant /api/status response shape", () => {
    const raw = {
      backends: [{ kind: "GPU", name: "Vulkan0" }],
      engine: "whisper-local",
      gpuOffload: "verified",
      loadedModel: {
        file: "ggml-large-v3-turbo-q8_0.bin",
        id: "QuantizedLargeTurbo",
        integrity: { state: "verified" },
        path: "/models/stt/ggml-large-v3-turbo-q8_0.bin",
      },
      modelIntegrity: { state: "verified" },
      probeRealtimeFactor: 1.917409062385559,
      requireGpu: false,
      uptimeSecs: 1826,
      version: "0.1.0",
    };

    expect(parseSttServerStatus(raw)).toEqual({
      engine: "whisper-local",
      gpuOffload: "verified",
      loadedModel: { id: "QuantizedLargeTurbo", file: "ggml-large-v3-turbo-q8_0.bin" },
      version: "0.1.0",
    });
  });

  test("handles no model loaded yet (loadedModel: null)", () => {
    const raw = {
      engine: "whisper-local",
      gpuOffload: "cpu",
      loadedModel: null,
      version: "0.1.0",
    };

    expect(parseSttServerStatus(raw)).toEqual({
      engine: "whisper-local",
      gpuOffload: "cpu",
      loadedModel: null,
      version: "0.1.0",
    });
  });

  test("returns null for a response missing the required fields", () => {
    expect(parseSttServerStatus({ ok: true })).toBeNull();
  });

  test("returns null for non-object JSON", () => {
    expect(parseSttServerStatus("nope")).toBeNull();
    expect(parseSttServerStatus(null)).toBeNull();
    expect(parseSttServerStatus(42)).toBeNull();
  });
});

describe("fetchSttServerStatus", () => {
  test("returns ok:false without making a request when the base URL is empty", async () => {
    const result = await fetchSttServerStatus("", "");
    expect(result).toEqual({ ok: false, error: "Enter a server URL first." });
    expect(fetchMock).not.toHaveBeenCalled();
  });

  test("returns ok:false for an unparsable base URL", async () => {
    const result = await fetchSttServerStatus("not a url", "");
    expect(result.ok).toBe(false);
    expect(fetchMock).not.toHaveBeenCalled();
  });

  test("hits /api/status on the origin and reports success", async () => {
    fetchMock.mockResolvedValueOnce({
      ok: true,
      status: 200,
      json: async () => ({
        engine: "whisper-local",
        gpuOffload: "verified",
        loadedModel: { id: "QuantizedLargeTurbo", file: "ggml-large-v3-turbo-q8_0.bin" },
        version: "0.1.0",
      }),
    });

    const result = await fetchSttServerStatus(
      "http://192.168.0.91:8383/v1",
      "",
    );

    expect(fetchMock).toHaveBeenCalledWith(
      "http://192.168.0.91:8383/api/status",
      expect.objectContaining({ method: "GET" }),
    );
    expect(result).toEqual({
      ok: true,
      status: {
        engine: "whisper-local",
        gpuOffload: "verified",
        loadedModel: { id: "QuantizedLargeTurbo", file: "ggml-large-v3-turbo-q8_0.bin" },
        version: "0.1.0",
      },
    });
  });

  test("sends a bearer token when an API key is configured", async () => {
    fetchMock.mockResolvedValueOnce({
      ok: true,
      status: 200,
      json: async () => ({ engine: "whisper-local", gpuOffload: "cpu", loadedModel: null }),
    });

    await fetchSttServerStatus("http://192.168.0.91:8383/v1", "secret-token");

    expect(fetchMock).toHaveBeenCalledWith(
      expect.any(String),
      expect.objectContaining({
        headers: expect.objectContaining({ Authorization: "Bearer secret-token" }),
      }),
    );
  });

  test("omits the Authorization header when no API key is configured", async () => {
    fetchMock.mockResolvedValueOnce({
      ok: true,
      status: 200,
      json: async () => ({ engine: "whisper-local", gpuOffload: "cpu", loadedModel: null }),
    });

    await fetchSttServerStatus("http://192.168.0.91:8383/v1", "");

    const [, options] = fetchMock.mock.calls[0];
    expect(options.headers).not.toHaveProperty("Authorization");
  });

  test("reports a clear failure for a non-2xx response", async () => {
    fetchMock.mockResolvedValueOnce({ ok: false, status: 401 });

    const result = await fetchSttServerStatus("http://192.168.0.91:8383/v1", "wrong");

    expect(result).toEqual({ ok: false, error: "Server responded with 401" });
  });

  test("reports a clear failure when the server is unreachable", async () => {
    fetchMock.mockRejectedValueOnce(new Error("connection refused"));

    const result = await fetchSttServerStatus("http://192.168.0.91:9999/v1", "");

    expect(result).toEqual({ ok: false, error: "connection refused" });
  });

  test("reports a clear failure for a response that isn't a Notare STT server", async () => {
    fetchMock.mockResolvedValueOnce({
      ok: true,
      status: 200,
      json: async () => ({ hello: "world" }),
    });

    const result = await fetchSttServerStatus("http://192.168.0.91:8383/v1", "");

    expect(result.ok).toBe(false);
  });
});

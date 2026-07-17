import { fetch as tauriFetch } from "@tauri-apps/plugin-http";

/**
 * "Test connection" for a self-hosted companion STT server
 * (docs/stt-server-design.md, issue #14 — the "Custom" STT provider). Hits
 * the server's `GET /api/status` (apps/stt-server/src/admin/status.rs),
 * which always answers 200 (even with no model loaded) and reports which
 * whisper engine/model is active and whether GPU offload was verified.
 *
 * Verified live against a real companion server on coruscant
 * (http://192.168.0.91:8383/api/status) during this change; real response
 * shape:
 *   {"engine":"whisper-local","gpuOffload":"verified",
 *    "loadedModel":{"id":"QuantizedLargeTurbo","file":"ggml-large-v3-turbo-q8_0.bin",...},
 *    "version":"0.1.0", ...}
 */

export type SttServerLoadedModel = {
  id: string;
  file?: string;
};

export type SttServerStatus = {
  engine: string;
  gpuOffload: string;
  loadedModel: SttServerLoadedModel | null;
  version?: string;
};

export type SttConnectionTestResult =
  | { ok: true; status: SttServerStatus }
  | { ok: false; error: string };

const REQUEST_TIMEOUT_MS = 5000;

/**
 * `/api/status` lives at the server root, not under whatever path the user's
 * `base_url` happens to end in (typically `.../v1`, since that's what the
 * live/batch STT calls need — see docs/stt-server-design.md §5). So this
 * derives the status URL from the origin only, ignoring the base URL's path.
 */
export function getSttServerStatusUrl(baseUrl: string): string {
  const url = new URL(baseUrl);
  return `${url.protocol}//${url.host}/api/status`;
}

export function parseSttServerStatus(json: unknown): SttServerStatus | null {
  if (typeof json !== "object" || json === null) {
    return null;
  }

  const record = json as Record<string, unknown>;
  const engine = record.engine;
  const gpuOffload = record.gpuOffload;

  if (typeof engine !== "string" || typeof gpuOffload !== "string") {
    return null;
  }

  let loadedModel: SttServerLoadedModel | null = null;
  if (typeof record.loadedModel === "object" && record.loadedModel !== null) {
    const model = record.loadedModel as Record<string, unknown>;
    if (typeof model.id === "string") {
      loadedModel = {
        id: model.id,
        file: typeof model.file === "string" ? model.file : undefined,
      };
    }
  }

  return {
    engine,
    gpuOffload,
    loadedModel,
    version: typeof record.version === "string" ? record.version : undefined,
  };
}

export async function fetchSttServerStatus(
  baseUrl: string,
  apiKey: string,
): Promise<SttConnectionTestResult> {
  const trimmedBaseUrl = baseUrl.trim();
  if (!trimmedBaseUrl) {
    return { ok: false, error: "Enter a server URL first." };
  }

  let url: string;
  try {
    url = getSttServerStatusUrl(trimmedBaseUrl);
  } catch {
    return { ok: false, error: "That doesn't look like a valid URL." };
  }

  const headers: Record<string, string> = {};
  const trimmedApiKey = apiKey.trim();
  if (trimmedApiKey.length > 0) {
    headers.Authorization = `Bearer ${trimmedApiKey}`;
  }

  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), REQUEST_TIMEOUT_MS);

  try {
    const response = await tauriFetch(url, {
      method: "GET",
      headers,
      signal: controller.signal,
    });

    if (!response.ok) {
      return { ok: false, error: `Server responded with ${response.status}` };
    }

    const json = await response.json();
    const status = parseSttServerStatus(json);
    if (!status) {
      return {
        ok: false,
        error: "Connected, but the response didn't look like a Notare STT server.",
      };
    }

    return { ok: true, status };
  } catch (error) {
    if (error instanceof Error && error.name === "AbortError") {
      return { ok: false, error: "Timed out waiting for a response." };
    }
    return {
      ok: false,
      error: error instanceof Error ? error.message : "Could not connect.",
    };
  } finally {
    clearTimeout(timeout);
  }
}

import { fetch as tauriFetch } from "@tauri-apps/plugin-http";

import { getSttServerOrigin } from "./connection-test";

/**
 * Model inventory + management for the "Custom" STT provider pointed at a
 * self-hosted companion Notare STT server (docs/stt-server-design.md, issue
 * #14). The admin API lives at the server origin (same place as `/api/status`
 * — see connection-test.ts), not under the `.../v1` base URL path the
 * live/batch STT calls use. All endpoints are token-gated with the same
 * `Authorization: Bearer <apiKey>` header as `/api/status` (the key is
 * optional when the server's bearer token is off).
 */

export type CustomSttModelIntegrity =
  | "verified"
  | "notInstalled"
  | "corrupt"
  | "presentUnverified";

export type CustomSttModel = {
  id: string;
  displayName: string;
  description: string;
  sizeBytes: number;
  englishOnly: boolean;
  active: boolean;
  installed: boolean;
  corrupt: boolean;
  // True when the server omitted `integrity` or reported a state we don't
  // recognize — we can't confidently claim installed/not-installed, so the
  // row renders neutrally instead of offering a misleading Download CTA.
  unknown: boolean;
};

export type ListCustomSttModelsResult =
  | { ok: true; models: CustomSttModel[] }
  | { ok: false; error: string };

export type CustomSttActionResult =
  | { ok: true; alreadyInstalled?: boolean }
  | { ok: false; error: string };

export type CustomSttProgressResult = {
  percent: number | null;
  complete: boolean;
  failed: boolean;
  detail?: string;
};

const REQUEST_TIMEOUT_MS = 8000;

export function getCustomSttModelsUrl(baseUrl: string): string {
  return `${getSttServerOrigin(baseUrl)}/api/models`;
}

export function getCustomSttModelUrl(
  baseUrl: string,
  id: string,
  action: "download" | "activate" | "progress",
): string {
  return `${getSttServerOrigin(baseUrl)}/api/models/${encodeURIComponent(id)}/${action}`;
}

function authHeaders(apiKey: string): Record<string, string> {
  const trimmedApiKey = apiKey.trim();
  const headers: Record<string, string> = {};
  if (trimmedApiKey.length > 0) {
    headers.Authorization = `Bearer ${trimmedApiKey}`;
  }
  return headers;
}

// The server serializes `ModelIntegrity` as a serde-tagged object, not a bare
// string: `{ "state": "verified" }` / `{ "state": "corrupt", "detail": "…" }`
// (crates/model-downloader/src/integrity.rs derives
// `#[serde(tag = "state", content = "detail")]`). Read `.state`; also tolerate a
// plain string so an older/alternate server shape still classifies correctly.
function readIntegrityState(raw: unknown): CustomSttModelIntegrity | undefined {
  if (typeof raw === "string") {
    return raw as CustomSttModelIntegrity;
  }
  if (typeof raw === "object" && raw !== null) {
    const state = (raw as Record<string, unknown>).state;
    if (typeof state === "string") {
      return state as CustomSttModelIntegrity;
    }
  }
  return undefined;
}

function parseCustomSttModel(raw: unknown): CustomSttModel | null {
  if (typeof raw !== "object" || raw === null) {
    return null;
  }

  const record = raw as Record<string, unknown>;
  if (typeof record.id !== "string") {
    return null;
  }

  const integrity = readIntegrityState(record.integrity);
  // "installed" covers verified files and present-but-unverified ones (still
  // usable); "notInstalled" needs a download; "corrupt" is re-downloadable.
  const installed =
    integrity === "verified" || integrity === "presentUnverified";
  const corrupt = integrity === "corrupt";
  // Absent (`undefined`/`null`) or unrecognized integrity is "unknown": neither
  // installed nor confidently not-installed, so it must not render as Download.
  const unknown = !installed && !corrupt && integrity !== "notInstalled";

  return {
    id: record.id,
    displayName:
      typeof record.displayName === "string" ? record.displayName : record.id,
    description:
      typeof record.description === "string" ? record.description : "",
    sizeBytes: typeof record.sizeBytes === "number" ? record.sizeBytes : 0,
    englishOnly: record.englishOnly === true,
    active: record.active === true,
    installed,
    corrupt,
    unknown,
  };
}

export function parseCustomSttModels(json: unknown): CustomSttModel[] {
  if (typeof json !== "object" || json === null) {
    return [];
  }

  const record = json as Record<string, unknown>;
  if (!Array.isArray(record.models)) {
    return [];
  }

  return record.models
    .map(parseCustomSttModel)
    .filter((model): model is CustomSttModel => model !== null);
}

export async function listCustomSttModels(
  baseUrl: string,
  apiKey: string,
  signal?: AbortSignal,
): Promise<ListCustomSttModelsResult> {
  const trimmedBaseUrl = baseUrl.trim();
  if (!trimmedBaseUrl) {
    return { ok: false, error: "Enter a server URL first." };
  }

  let url: string;
  try {
    url = getCustomSttModelsUrl(trimmedBaseUrl);
  } catch {
    return { ok: false, error: "That doesn't look like a valid URL." };
  }

  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), REQUEST_TIMEOUT_MS);
  // Thread react-query's AbortSignal in so a stale in-flight request (the URL
  // or key changed mid-flight) is cancelled and its result ignored, keeping the
  // loading/empty/error states mutually exclusive. We forward external aborts
  // onto our own controller so the timeout still applies independently.
  const onExternalAbort = () => controller.abort();
  if (signal) {
    if (signal.aborted) {
      controller.abort();
    } else {
      signal.addEventListener("abort", onExternalAbort, { once: true });
    }
  }

  try {
    const response = await tauriFetch(url, {
      method: "GET",
      headers: authHeaders(apiKey),
      signal: controller.signal,
    });

    if (!response.ok) {
      return { ok: false, error: `Server responded with ${response.status}` };
    }

    const json = await response.json();
    const models = parseCustomSttModels(json);
    return { ok: true, models };
  } catch (error) {
    // Aborts surface as a DOMException named "AbortError", which is NOT reliably
    // `instanceof Error` (webview/jsdom differ), so match on `.name` directly.
    const name =
      typeof error === "object" && error !== null
        ? (error as { name?: unknown }).name
        : undefined;
    if (name === "AbortError") {
      // An abort triggered by the caller (not our timeout) means this query is
      // stale; react-query discards the result anyway, so label it as such.
      if (signal?.aborted) {
        return { ok: false, error: "Cancelled." };
      }
      return { ok: false, error: "Timed out waiting for a response." };
    }
    return {
      ok: false,
      error: error instanceof Error ? error.message : "Could not connect.",
    };
  } finally {
    clearTimeout(timeout);
    if (signal) {
      signal.removeEventListener("abort", onExternalAbort);
    }
  }
}

export async function downloadCustomSttModel(
  baseUrl: string,
  apiKey: string,
  id: string,
): Promise<CustomSttActionResult> {
  let url: string;
  try {
    url = getCustomSttModelUrl(baseUrl, id, "download");
  } catch {
    return { ok: false, error: "That doesn't look like a valid URL." };
  }

  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), REQUEST_TIMEOUT_MS);

  try {
    const response = await tauriFetch(url, {
      method: "POST",
      headers: authHeaders(apiKey),
      signal: controller.signal,
    });

    // 202 = download started, 200 = already installed (no-op).
    if (response.status === 200) {
      return { ok: true, alreadyInstalled: true };
    }
    if (response.status === 202) {
      return { ok: true };
    }
    if (response.status === 409) {
      return { ok: false, error: "Download already in progress." };
    }
    if (response.status === 404) {
      return { ok: false, error: "Unknown model." };
    }
    return { ok: false, error: `Server responded with ${response.status}` };
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

export async function activateCustomSttModel(
  baseUrl: string,
  apiKey: string,
  id: string,
): Promise<CustomSttActionResult> {
  let url: string;
  try {
    url = getCustomSttModelUrl(baseUrl, id, "activate");
  } catch {
    return { ok: false, error: "That doesn't look like a valid URL." };
  }

  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), REQUEST_TIMEOUT_MS);

  try {
    const response = await tauriFetch(url, {
      method: "POST",
      headers: authHeaders(apiKey),
      signal: controller.signal,
    });

    if (response.ok) {
      return { ok: true };
    }
    if (response.status === 404) {
      return { ok: false, error: "Unknown model." };
    }
    return { ok: false, error: `Server responded with ${response.status}` };
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

// Matches the server's `progress_snapshot` (apps/stt-server/src/admin/models.rs):
//   GET /api/models/{id}/progress → { "id", "progress": { "status", "percent", "detail" } }
//   status ∈ downloading | completed | failed | idle | corrupt; percent is 0-100.
// The per-entry `progress` embedded in GET /api/models is the same inner object,
// so this accepts either the wrapped envelope or a bare inner object.
export function parseCustomSttProgress(json: unknown): CustomSttProgressResult {
  const outer =
    typeof json === "object" && json !== null
      ? (json as Record<string, unknown>)
      : {};
  const progress =
    typeof outer.progress === "object" && outer.progress !== null
      ? (outer.progress as Record<string, unknown>)
      : outer;

  const status =
    typeof progress.status === "string" ? progress.status : undefined;

  let percent: number | null = null;
  if (typeof progress.percent === "number") {
    percent = Math.max(0, Math.min(100, Math.round(progress.percent)));
  }

  if (status === "failed" || status === "corrupt") {
    return {
      percent,
      complete: false,
      failed: true,
      detail: typeof progress.detail === "string" ? progress.detail : undefined,
    };
  }

  const complete =
    status === "completed" || (percent !== null && percent >= 100);

  return { percent, complete, failed: false };
}

export async function fetchCustomSttModelProgress(
  baseUrl: string,
  apiKey: string,
  id: string,
): Promise<CustomSttProgressResult | null> {
  let url: string;
  try {
    url = getCustomSttModelUrl(baseUrl, id, "progress");
  } catch {
    return null;
  }

  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), REQUEST_TIMEOUT_MS);

  try {
    const response = await tauriFetch(url, {
      method: "GET",
      headers: authHeaders(apiKey),
      signal: controller.signal,
    });
    if (!response.ok) {
      return null;
    }
    return parseCustomSttProgress(await response.json());
  } catch {
    return null;
  } finally {
    clearTimeout(timeout);
  }
}

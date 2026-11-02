import { fetch as tauriFetch } from "@tauri-apps/plugin-http";

import { sonnerToast } from "@hypr/ui/components/ui/toast";

import {
  commands as llmAuthCommands,
  type LlmAuthProvider,
  type LlmAuthToken,
} from "./llm-auth-client";

export async function extractVendorErrorMessage(
  response: Response,
): Promise<string> {
  try {
    const clone = response.clone();
    const data = (await clone.json()) as Record<string, unknown>;
    if (typeof data === "object" && data !== null) {
      if (typeof data.error === "string") return data.error;
      const errObj = data.error as Record<string, unknown> | undefined;
      if (errObj && typeof errObj.message === "string") return errObj.message;
      if (typeof data.message === "string") return data.message;
      if (typeof data.detail === "string") return data.detail;
    }
    const text = await response.clone().text();
    if (text && text.length < 300) return text;
  } catch {
    // fallback to status text
  }
  return response.statusText || `Request failed with status ${response.status}`;
}

export async function handleVendorErrorToast(
  response: Response,
): Promise<void> {
  const message = await extractVendorErrorMessage(response);
  sonnerToast.error(message);
}

export type SubscriptionFetchOptions = {
  provider: LlmAuthProvider;
  injectHeaders?: (
    token: string,
    extra: Partial<Record<string, string>>,
  ) => Record<string, string>;
  transformBody?: (bodyText: string) => string;
  rewriteUrl?: (url: string) => string;
  transformResponse?: (response: Response) => Promise<Response> | Response;
  fetchFn?: typeof fetch;
  getToken?: () => Promise<LlmAuthToken>;
};

export function createSubscriptionFetch(
  options: SubscriptionFetchOptions,
): typeof fetch {
  const baseFetch = options.fetchFn ?? tauriFetch;

  return async (
    input: RequestInfo | URL,
    init?: RequestInit,
  ): Promise<Response> => {
    let token: LlmAuthToken;
    if (options.getToken) {
      token = await options.getToken();
    } else {
      const tokenRes = await llmAuthCommands.accessToken(options.provider);
      if (tokenRes.status !== "ok") {
        sonnerToast.error(
          "Authentication expired or missing. Please sign in again.",
        );
        throw new Error(tokenRes.error);
      }
      token = tokenRes.data;
    }

    let url =
      typeof input === "string"
        ? input
        : input instanceof URL
          ? input.toString()
          : input.url;
    if (options.rewriteUrl) {
      url = options.rewriteUrl(url);
    }

    const headers = new Headers(init?.headers);
    if (options.injectHeaders) {
      const injected = options.injectHeaders(token.token, token.extra);
      for (const [key, value] of Object.entries(injected)) {
        headers.set(key, value);
      }
    }

    let body = init?.body;
    if (options.transformBody && typeof body === "string") {
      body = options.transformBody(body);
    }

    const requestInit: RequestInit = {
      ...init,
      headers,
      body,
    };

    let response = await baseFetch(url, requestInit);

    // 401 handling: retry once after calling accessToken again
    if (response.status === 401) {
      try {
        let refreshed: LlmAuthToken | null = null;
        if (options.getToken) {
          refreshed = await options.getToken();
        } else {
          const res = await llmAuthCommands.accessToken(options.provider);
          if (res.status === "ok") {
            refreshed = res.data;
          }
        }

        if (refreshed) {
          const retryHeaders = new Headers(init?.headers);
          if (options.injectHeaders) {
            const injected = options.injectHeaders(
              refreshed.token,
              refreshed.extra,
            );
            for (const [key, value] of Object.entries(injected)) {
              retryHeaders.set(key, value);
            }
          }

          const retryInit: RequestInit = {
            ...init,
            headers: retryHeaders,
            body,
          };

          const retryResponse = await baseFetch(url, retryInit);
          if (retryResponse.status !== 401) {
            if (retryResponse.status === 403 || retryResponse.status === 429) {
              await handleVendorErrorToast(retryResponse);
            }
            return options.transformResponse
              ? await options.transformResponse(retryResponse)
              : retryResponse;
          }
          response = retryResponse;
        }
      } catch {
        // Refresh attempt failed, continue to surface error toast
      }

      const vendorMsg = await extractVendorErrorMessage(response);
      sonnerToast.error(`${vendorMsg}. Please sign in again.`);
      return response;
    }

    // 403 / 429 handling: toast vendor message
    if (response.status === 403 || response.status === 429) {
      await handleVendorErrorToast(response);
      return response;
    }

    return options.transformResponse
      ? await options.transformResponse(response)
      : response;
  };
}

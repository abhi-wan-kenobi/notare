import { fetch as tauriFetch } from "@tauri-apps/plugin-http";

import { sonnerToast } from "@hypr/ui/components/ui/toast";

import {
  commands as llmAuthCommands,
  type LlmAuthProvider,
  type LlmAuthToken,
} from "./llm-auth-client";

/**
 * tauri-plugin-http stamps every request with `Origin: tauri://localhost`.
 * Anthropic rejects OAuth calls that carry an Origin ("CORS requests are not
 * allowed for this Organization"); the official CLIs never send one. With the
 * plugin's `unsafe-headers` feature (enabled in the workspace), an empty Origin
 * makes it drop the header entirely.
 */
export function stripBrowserOrigin(headers: Headers): void {
  headers.set("Origin", "");
}

export async function extractVendorErrorMessage(
  response: Response,
): Promise<string> {
  try {
    const text = await response.clone().text();
    if (text) {
      try {
        const data = JSON.parse(text) as Record<string, unknown>;
        if (typeof data === "object" && data !== null) {
          if (typeof data.error === "string") return data.error;
          const errObj = data.error as Record<string, unknown> | undefined;
          if (errObj && typeof errObj.message === "string")
            return errObj.message;
          if (typeof data.message === "string") return data.message;
          if (typeof data.detail === "string") return data.detail;
        }
      } catch {
        // Fallback to text if not JSON
      }
      if (text.length <= 300) return text;
      return `${text.slice(0, 300)}...`;
    }
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

async function bufferBodyText(
  body: BodyInit | null | undefined,
): Promise<string | undefined> {
  if (body === null || body === undefined) {
    return undefined;
  }
  if (typeof body === "string") {
    return body;
  }
  if (body instanceof ArrayBuffer) {
    return new TextDecoder().decode(body);
  }
  if (ArrayBuffer.isView(body)) {
    return new TextDecoder().decode(
      body.buffer.slice(body.byteOffset, body.byteOffset + body.byteLength),
    );
  }
  if (body instanceof Blob) {
    return await body.text();
  }
  if (typeof (body as ReadableStream).getReader === "function") {
    const reader = (body as ReadableStream<Uint8Array>).getReader();
    const decoder = new TextDecoder();
    let text = "";
    while (true) {
      const { done, value } = await reader.read();
      if (done) break;
      if (value) {
        text += decoder.decode(value, { stream: true });
      }
    }
    text += decoder.decode();
    return text;
  }
  if (body instanceof FormData || body instanceof URLSearchParams) {
    return await new Response(body).text();
  }
  return await new Response(body).text();
}

export type SubscriptionFetchOptions = {
  provider: LlmAuthProvider;
  injectHeaders?: (
    token: string,
    extra: Partial<Record<string, string>>,
  ) => Record<string, string>;
  /** Headers the underlying SDK adds that must not reach the vendor. */
  dropHeaders?: string[];
  transformBody?: (
    bodyText: string,
    extra?: Partial<Record<string, string>>,
    token?: LlmAuthToken,
  ) => string | Promise<string>;
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
    if (
      typeof input === "object" &&
      !(input instanceof URL) &&
      "headers" in input &&
      !init?.headers
    ) {
      for (const [key, value] of (input as Request).headers.entries()) {
        headers.set(key, value);
      }
    }
    if (options.injectHeaders) {
      const injected = options.injectHeaders(token.token, token.extra);
      for (const [key, value] of Object.entries(injected)) {
        headers.set(key, value);
      }
    }
    for (const name of options.dropHeaders ?? []) headers.delete(name);
    stripBrowserOrigin(headers);

    let bodyInput: BodyInit | null | undefined = init?.body;
    if (
      bodyInput === undefined &&
      typeof input === "object" &&
      !(input instanceof URL) &&
      "body" in input
    ) {
      bodyInput = (input as Request).body;
    }
    const bufferedBody = await bufferBodyText(bodyInput);

    let body: BodyInit | undefined = bufferedBody;
    if (options.transformBody && bufferedBody !== undefined) {
      body = await options.transformBody(bufferedBody, token.extra, token);
    }

    const requestInit: RequestInit = {
      ...init,
      headers,
      body,
    };

    let response = await baseFetch(url, requestInit);

    // 401 handling: retry once after calling accessToken again
    if (response.status === 401) {
      const originalVendorMsg = await extractVendorErrorMessage(response);

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
          if (
            typeof input === "object" &&
            !(input instanceof URL) &&
            "headers" in input &&
            !init?.headers
          ) {
            for (const [key, value] of (input as Request).headers.entries()) {
              retryHeaders.set(key, value);
            }
          }
          if (options.injectHeaders) {
            const injected = options.injectHeaders(
              refreshed.token,
              refreshed.extra,
            );
            for (const [key, value] of Object.entries(injected)) {
              retryHeaders.set(key, value);
            }
          }
          for (const name of options.dropHeaders ?? []) {
            retryHeaders.delete(name);
          }
          stripBrowserOrigin(retryHeaders);

          let retryBody: BodyInit | undefined = bufferedBody;
          if (options.transformBody && bufferedBody !== undefined) {
            retryBody = await options.transformBody(
              bufferedBody,
              refreshed.extra,
              refreshed,
            );
          }

          const retryInit: RequestInit = {
            ...init,
            headers: retryHeaders,
            body: retryBody,
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
      } catch (err) {
        console.error("Token refresh or retry failed:", err);
      }

      sonnerToast.error(`${originalVendorMsg}. Please sign in again.`);
      return options.transformResponse
        ? await options.transformResponse(response)
        : response;
    }

    // 403 / 429 handling: toast vendor message
    if (response.status === 403 || response.status === 429) {
      await handleVendorErrorToast(response);
      return options.transformResponse
        ? await options.transformResponse(response)
        : response;
    }

    return options.transformResponse
      ? await options.transformResponse(response)
      : response;
  };
}

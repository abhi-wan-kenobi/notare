import { createOpenAI } from "@ai-sdk/openai";

import {
  DEFAULT_CATALOG,
  type LlmAuthCatalogProvider,
  type LlmAuthToken,
} from "./llm-auth-client";
import { createSubscriptionFetch } from "./subscription-fetch";

/** Offline fallback; the picker reads the catalog's `models` first. */
export const OPENAI_CODEX_SUBSCRIPTION_MODELS = [
  "gpt-6.1-sol",
  "gpt-6.1-terra",
  "gpt-5.6-luna",
] as const;

export type OpenAICodexSubscriptionModelId =
  | (typeof OPENAI_CODEX_SUBSCRIPTION_MODELS)[number]
  | (string & {});

/**
 * The ChatGPT backend rejects any request that is not `store: false` and
 * `stream: true` ("Store must be set to false" / "Stream must be set to
 * true"), whatever the caller asked for.
 */
export function transformCodexRequestBody(bodyText: string): string {
  try {
    const body = JSON.parse(bodyText) as Record<string, unknown>;
    if (typeof body !== "object" || body === null || Array.isArray(body)) {
      return bodyText;
    }
    body.store = false;
    body.stream = true;
    return JSON.stringify(body);
  } catch {
    return bodyText;
  }
}

function callerWantedStream(requestBody: string | undefined): boolean {
  if (!requestBody) return false;
  try {
    return (JSON.parse(requestBody) as { stream?: unknown }).stream === true;
  } catch {
    return false;
  }
}

type SseEvent = { type?: string; [key: string]: unknown };

function parseSseEvents(text: string): SseEvent[] {
  const events: SseEvent[] = [];
  for (const block of text.split(/\r?\n\r?\n/)) {
    const data = block
      .split(/\r?\n/)
      .filter((line) => line.startsWith("data:"))
      .map((line) => line.slice(5).replace(/^ /, ""))
      .join("\n");
    if (!data || data === "[DONE]") continue;
    try {
      const parsed = JSON.parse(data) as unknown;
      if (parsed && typeof parsed === "object") {
        events.push(parsed as SseEvent);
      }
    } catch {
      // Ignore non-JSON keep-alives.
    }
  }
  return events;
}

/**
 * Collapses a Responses API SSE stream into the JSON a non-streaming call
 * returns. A `response.completed`/`response.incomplete` event carries the
 * final object and wins over any `error` event, so a late error cannot turn
 * a finished response into a 502; if the backend leaves its `output` empty
 * the streamed output items fill it in. Without such a terminal event an
 * `error` event maps to a 502, then `response.failed` is passed through, and
 * with no terminal at all the text deltas are concatenated.
 */
export function reassembleCodexResponse(sseText: string): {
  status: number;
  body: unknown;
} {
  const events = parseSseEvents(sseText);

  const finished = [...events]
    .reverse()
    .find(
      (e) =>
        (e.type === "response.completed" || e.type === "response.incomplete") &&
        e.response &&
        typeof e.response === "object",
    );

  const errorEvent = finished
    ? undefined
    : events.find((e) => e.type === "error");
  if (errorEvent) {
    return {
      status: 502,
      body: {
        error: {
          message:
            typeof errorEvent.message === "string"
              ? errorEvent.message
              : "ChatGPT backend reported an error",
          code: errorEvent.code ?? null,
        },
      },
    };
  }

  const items: unknown[] = [];
  for (const e of events) {
    if (e.type === "response.output_item.done" && e.item) {
      const index =
        typeof e.output_index === "number" ? e.output_index : items.length;
      items[index] = e.item;
    }
  }
  const streamedItems = items.filter((item) => item !== undefined);

  const terminal =
    finished ??
    [...events]
      .reverse()
      .find(
        (e) =>
          e.type === "response.failed" &&
          e.response &&
          typeof e.response === "object",
      );
  if (terminal) {
    const response = { ...(terminal.response as Record<string, unknown>) };
    if (
      (!Array.isArray(response.output) || response.output.length === 0) &&
      streamedItems.length > 0
    ) {
      response.output = streamedItems;
    }
    return { status: 200, body: response };
  }

  const text = events
    .filter((e) => e.type === "response.output_text.delta")
    .map((e) => (typeof e.delta === "string" ? e.delta : ""))
    .join("");
  const created = events.find((e) => e.type === "response.created")
    ?.response as Record<string, unknown> | undefined;
  return {
    status: 200,
    body: {
      ...created,
      object: "response",
      status: "completed",
      output:
        streamedItems.length > 0
          ? streamedItems
          : [
              {
                type: "message",
                role: "assistant",
                content: [{ type: "output_text", text, annotations: [] }],
              },
            ],
    },
  };
}

/**
 * Streaming callers (`streamText`) get the SSE untouched. Callers that did
 * not ask for a stream (`generateText`/`generateObject`) get the reassembled
 * JSON, since `transformCodexRequestBody` forced streaming upstream.
 */
export async function transformCodexResponse(
  response: Response,
  context: { requestBody: string | undefined },
): Promise<Response> {
  if (!response.ok || callerWantedStream(context.requestBody)) {
    return response;
  }
  // The Codex backend omits content-type unless the request sent
  // `Accept: text/event-stream` (measured 2026-11-03), so sniff the body
  // instead of trusting the header. We forced `stream: true` upstream, so a
  // non-JSON body here is always SSE.
  const text = await response.text();
  const looksLikeSse = /^\s*(event|data):/.test(text);
  if (!looksLikeSse) {
    return new Response(text, {
      status: response.status,
      statusText: response.statusText,
      headers: response.headers,
    });
  }

  const { status, body } = reassembleCodexResponse(text);
  const headers = new Headers(response.headers);
  headers.delete("content-length");
  headers.delete("content-encoding");
  headers.set("content-type", "application/json");
  return new Response(JSON.stringify(body), {
    status,
    statusText: status === 200 ? response.statusText : "Bad Gateway",
    headers,
  });
}

export type OpenAICodexSubscriptionOptions = {
  fetchFn?: typeof fetch;
  getToken?: () => Promise<LlmAuthToken>;
  catalog?: LlmAuthCatalogProvider;
};

export function createOpenAICodexSubscriptionFetch(
  options: OpenAICodexSubscriptionOptions = {},
): typeof fetch {
  const catalog =
    options.catalog ??
    DEFAULT_CATALOG.providers.find((p) => p.id === "openai_codex");
  const betaHeaders = catalog?.beta_headers ?? {
    "OpenAI-Beta": "responses=experimental",
  };

  return createSubscriptionFetch({
    provider: "openai_codex",
    fetchFn: options.fetchFn,
    getToken: options.getToken,
    transformBody: transformCodexRequestBody,
    transformResponse: transformCodexResponse,
    injectHeaders: (token, extra) => {
      const headers: Record<string, string> = {
        Authorization: `Bearer ${token}`,
        // Every request is forced to stream; ask for it explicitly so the
        // backend also labels the response.
        Accept: "text/event-stream",
        ...betaHeaders,
      };
      if (extra?.chatgpt_account_id) {
        headers["chatgpt-account-id"] = extra.chatgpt_account_id;
      }
      return headers;
    },
  });
}

export function createOpenAICodexSubscriptionModel(
  modelId: string,
  options: OpenAICodexSubscriptionOptions = {},
) {
  const catalog =
    options.catalog ??
    DEFAULT_CATALOG.providers.find((p) => p.id === "openai_codex");
  const baseURL = catalog?.base_url ?? "https://chatgpt.com/backend-api/codex";
  const customFetch = createOpenAICodexSubscriptionFetch(options);

  const provider = createOpenAI({
    baseURL,
    fetch: customFetch,
    apiKey: "subscription",
  });

  return provider.responses(modelId);
}

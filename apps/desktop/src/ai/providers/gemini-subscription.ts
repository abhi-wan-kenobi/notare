import { createGoogleGenerativeAI } from "@ai-sdk/google";

import {
  type LlmAuthCatalogProvider,
  type LlmAuthToken,
} from "./llm-auth-client";
import { createSubscriptionFetch } from "./subscription-fetch";

export const GEMINI_SUBSCRIPTION_MODELS = [
  "gemini-2.5-pro",
  "gemini-2.5-flash",
] as const;

export type GeminiSubscriptionModelId =
  | (typeof GEMINI_SUBSCRIPTION_MODELS)[number]
  | (string & {});

export function rewriteGeminiUrl(urlStr: string): string {
  try {
    const url = new URL(urlStr);
    const isStream = url.pathname.includes(":streamGenerateContent");
    const endpoint = isStream ? "streamGenerateContent" : "generateContent";
    const newUrl = new URL(
      `https://cloudcode-pa.googleapis.com/v1internal:${endpoint}`,
    );
    url.searchParams.forEach((val, key) => {
      newUrl.searchParams.set(key, val);
    });
    return newUrl.toString();
  } catch {
    return urlStr.replace(
      /https:\/\/generativelanguage\.googleapis\.com\/v1(beta)?\/models\/[^:]+:(streamGenerateContent|generateContent)/,
      "https://cloudcode-pa.googleapis.com/v1internal:$2",
    );
  }
}

export function wrapGeminiRequestBody(
  bodyText: string,
  model: string,
  project?: string,
): string {
  try {
    const parsed = JSON.parse(bodyText) as Record<string, unknown>;
    if (
      parsed &&
      typeof parsed === "object" &&
      !Array.isArray(parsed) &&
      "request" in parsed
    ) {
      if (project && !parsed.project) {
        parsed.project = project;
        return JSON.stringify(parsed);
      }
      return bodyText;
    }
    const wrapped: Record<string, unknown> = {
      model,
      ...(project ? { project } : {}),
      request: parsed,
    };
    return JSON.stringify(wrapped);
  } catch {
    return bodyText;
  }
}

export function transformSseStream(
  bodyStream: ReadableStream<Uint8Array>,
): ReadableStream<Uint8Array> {
  const reader = bodyStream.getReader();
  const decoder = new TextDecoder();
  const encoder = new TextEncoder();
  let buffer = "";

  const processLine = (
    line: string,
    controller: ReadableStreamDefaultController<Uint8Array>,
  ) => {
    const trimmed = line.trimEnd();
    if (trimmed.startsWith("data:")) {
      const dataStr = trimmed.slice(5).trim();
      if (dataStr && dataStr !== "[DONE]") {
        try {
          const parsed = JSON.parse(dataStr) as Record<string, unknown>;
          if (parsed && typeof parsed === "object" && "response" in parsed) {
            controller.enqueue(
              encoder.encode(`data: ${JSON.stringify(parsed.response)}\n\n`),
            );
            return;
          }
        } catch {
          // Pass through on JSON parse error
        }
      }
    }
    controller.enqueue(encoder.encode(`${line}\n`));
  };

  return new ReadableStream<Uint8Array>({
    async pull(controller) {
      while (true) {
        const { done, value } = await reader.read();
        if (done) {
          if (buffer.length > 0) {
            const lines = buffer.split("\n");
            for (const line of lines) {
              if (line) processLine(line, controller);
            }
          }
          controller.close();
          return;
        }

        buffer += decoder.decode(value, { stream: true });
        const lines = buffer.split("\n");
        buffer = lines.pop() ?? "";

        for (const line of lines) {
          processLine(line, controller);
        }
      }
    },
    cancel(reason) {
      return reader.cancel(reason);
    },
  });
}

export async function transformGeminiResponse(
  response: Response,
): Promise<Response> {
  if (!response.ok) {
    return response;
  }

  const contentType = response.headers.get("content-type") || "";
  if (contentType.includes("text/event-stream") && response.body) {
    const transformedStream = transformSseStream(response.body);
    const headers = new Headers(response.headers);
    headers.delete("content-length");
    headers.delete("content-encoding");
    return new Response(transformedStream, {
      status: response.status,
      statusText: response.statusText,
      headers,
    });
  }

  if (contentType.includes("application/json")) {
    try {
      const data = (await response.json()) as Record<string, unknown>;
      if (data && typeof data === "object" && "response" in data) {
        const headers = new Headers(response.headers);
        headers.delete("content-length");
        headers.delete("content-encoding");
        return new Response(JSON.stringify(data.response), {
          status: response.status,
          statusText: response.statusText,
          headers,
        });
      }
    } catch {
      // Pass through on parse error
    }
  }

  return response;
}

export type GeminiSubscriptionOptions = {
  fetchFn?: typeof fetch;
  getToken?: () => Promise<LlmAuthToken>;
  catalog?: LlmAuthCatalogProvider;
};

export function createGeminiSubscriptionFetch(
  modelId: string,
  options: GeminiSubscriptionOptions = {},
): typeof fetch {
  return createSubscriptionFetch({
    provider: "gemini",
    fetchFn: options.fetchFn,
    getToken: options.getToken,
    rewriteUrl: rewriteGeminiUrl,
    injectHeaders: (token) => ({
      Authorization: `Bearer ${token}`,
    }),
    transformBody: (bodyText, extra) => {
      const projectId = extra?.project_id;
      return wrapGeminiRequestBody(bodyText, modelId, projectId);
    },
    transformResponse: transformGeminiResponse,
  });
}

export function createGeminiSubscriptionModel(
  modelId: string,
  options: GeminiSubscriptionOptions = {},
) {
  const customFetch = createGeminiSubscriptionFetch(modelId, options);

  const provider = createGoogleGenerativeAI({
    fetch: customFetch,
    apiKey: "subscription",
  });

  return provider(modelId);
}

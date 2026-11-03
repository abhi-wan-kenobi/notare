import { beforeEach, describe, expect, it, vi } from "vitest";

import {
  createAnthropicSubscriptionFetch,
  transformAnthropicRequestBody,
} from "./anthropic-subscription";
import {
  createGeminiSubscriptionFetch,
  createGeminiSubscriptionModel,
  rewriteGeminiUrl,
  transformGeminiResponse,
  wrapGeminiRequestBody,
} from "./gemini-subscription";
import { subscriptionModelIds } from "./llm-auth-client";
import {
  createOpenAICodexSubscriptionFetch,
  reassembleCodexResponse,
  transformCodexRequestBody,
  transformCodexResponse,
} from "./openai-codex-subscription";
import {
  createSubscriptionFetch,
  extractVendorErrorMessage,
} from "./subscription-fetch";

const mocks = vi.hoisted(() => ({
  sonnerToastError: vi.fn(),
  sonnerToastInfo: vi.fn(),
}));

vi.mock("@hypr/ui/components/ui/toast", () => ({
  sonnerToast: {
    error: mocks.sonnerToastError,
    info: mocks.sonnerToastInfo,
  },
}));

describe("Anthropic Subscription Provider", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("transforms request body so first system block is catalog system_prefix and original prompt is second block", () => {
    const systemPrefix =
      "You are Claude Code, Anthropic's official CLI for Claude.";

    // Case 1: String system prompt
    const bodyWithString = JSON.stringify({
      model: "claude-sonnet-4-5",
      system: "Summarize this meeting note.",
      messages: [{ role: "user", content: "Hello" }],
    });

    const transformed1 = JSON.parse(
      transformAnthropicRequestBody(bodyWithString, systemPrefix),
    );
    expect(transformed1.system).toEqual([
      { type: "text", text: systemPrefix },
      { type: "text", text: "Summarize this meeting note." },
    ]);

    // Case 2: Array of system blocks
    const bodyWithArray = JSON.stringify({
      model: "claude-sonnet-4-5",
      system: [{ type: "text", text: "User system instruction" }],
      messages: [{ role: "user", content: "Hello" }],
    });

    const transformed2 = JSON.parse(
      transformAnthropicRequestBody(bodyWithArray, systemPrefix),
    );
    expect(transformed2.system).toEqual([
      { type: "text", text: systemPrefix },
      { type: "text", text: "User system instruction" },
    ]);

    // Case 3: Missing system prompt
    const bodyWithoutSystem = JSON.stringify({
      model: "claude-sonnet-4-5",
      messages: [{ role: "user", content: "Hello" }],
    });

    const transformed3 = JSON.parse(
      transformAnthropicRequestBody(bodyWithoutSystem, systemPrefix),
    );
    expect(transformed3.system).toEqual([{ type: "text", text: systemPrefix }]);

    // Case 4: Idempotent if prefix already exists
    const bodyWithPrefixAlready = JSON.stringify({
      model: "claude-sonnet-4-5",
      system: [
        { type: "text", text: systemPrefix },
        { type: "text", text: "Original" },
      ],
      messages: [],
    });
    const transformed4 = JSON.parse(
      transformAnthropicRequestBody(bodyWithPrefixAlready, systemPrefix),
    );
    expect(transformed4.system).toHaveLength(2);
    expect(transformed4.system[0].text).toBe(systemPrefix);
  });

  it("injects Authorization, beta headers, and system prefix into fetch request", async () => {
    let capturedUrl = "";
    let capturedInit: RequestInit | undefined;

    const fakeFetch: typeof fetch = async (url, init) => {
      capturedUrl = String(url);
      capturedInit = init;
      return new Response(JSON.stringify({ id: "msg_123", content: [] }), {
        status: 200,
        headers: { "Content-Type": "application/json" },
      });
    };

    const anthropicFetch = createAnthropicSubscriptionFetch({
      fetchFn: fakeFetch,
      getToken: async () => ({ token: "sk-ant-test-token", extra: {} }),
      catalog: {
        id: "anthropic",
        enabled: true,
        label: "Anthropic",
        beta_headers: {
          "anthropic-beta": "claude-code-20250219,oauth-2025-04-20",
        },
        system_prefix:
          "You are Claude Code, Anthropic's official CLI for Claude.",
        base_url: "https://api.anthropic.com/v1",
        notes: "",
      },
    });

    const originalBody = JSON.stringify({
      model: "claude-sonnet-4-5",
      system: "Original prompt",
      messages: [{ role: "user", content: "Hi" }],
    });

    await anthropicFetch("https://api.anthropic.com/v1/messages", {
      method: "POST",
      body: originalBody,
      headers: {
        "Content-Type": "application/json",
        // What @ai-sdk/anthropic adds; Anthropic rejects OAuth calls carrying these.
        "x-api-key": "subscription",
        "anthropic-dangerous-direct-browser-access": "true",
      },
    });

    expect(capturedUrl).toBe("https://api.anthropic.com/v1/messages");
    const headers = new Headers(capturedInit?.headers);
    expect(headers.get("Authorization")).toBe("Bearer sk-ant-test-token");
    expect(headers.get("anthropic-beta")).toBe(
      "claude-code-20250219,oauth-2025-04-20",
    );
    expect(headers.get("anthropic-version")).toBe("2023-06-01");
    expect(headers.get("x-api-key")).toBeNull();
    expect(headers.get("anthropic-dangerous-direct-browser-access")).toBeNull();
    // Empty Origin tells tauri-plugin-http (unsafe-headers) to drop its
    // automatic `Origin: tauri://localhost`, which triggers Anthropic's CORS block.
    expect(headers.get("Origin")).toBe("");

    const parsedBody = JSON.parse(String(capturedInit?.body));
    expect(parsedBody.system[0]).toEqual({
      type: "text",
      text: "You are Claude Code, Anthropic's official CLI for Claude.",
    });
    expect(parsedBody.system[1]).toEqual({
      type: "text",
      text: "Original prompt",
    });
  });

  it("retries on 401 and shows toast with vendor message on failure", async () => {
    let callCount = 0;
    let tokenCalls = 0;

    const fakeFetch: typeof fetch = async () => {
      callCount++;
      return new Response(
        JSON.stringify({ error: { message: "Token expired or revoked" } }),
        { status: 401, headers: { "Content-Type": "application/json" } },
      );
    };

    const anthropicFetch = createAnthropicSubscriptionFetch({
      fetchFn: fakeFetch,
      getToken: async () => {
        tokenCalls++;
        return { token: `token-attempt-${tokenCalls}`, extra: {} };
      },
    });

    const res = await anthropicFetch("https://api.anthropic.com/v1/messages", {
      method: "POST",
      body: "{}",
    });

    expect(res.status).toBe(401);
    expect(callCount).toBe(2); // Initial attempt + 1 retry
    expect(tokenCalls).toBe(2); // Initial get + retry refresh
    expect(mocks.sonnerToastError).toHaveBeenCalledWith(
      "Token expired or revoked. Please sign in again.",
    );
  });

  it("shows toast with vendor message on 403 or 429", async () => {
    const fakeFetch429: typeof fetch = async () => {
      return new Response(
        JSON.stringify({
          error: { message: "Rate limit reached. Please wait." },
        }),
        { status: 429, headers: { "Content-Type": "application/json" } },
      );
    };

    const anthropicFetch = createAnthropicSubscriptionFetch({
      fetchFn: fakeFetch429,
      getToken: async () => ({ token: "test-token", extra: {} }),
    });

    const res = await anthropicFetch("https://api.anthropic.com/v1/messages", {
      method: "POST",
      body: "{}",
    });

    expect(res.status).toBe(429);
    expect(mocks.sonnerToastError).toHaveBeenCalledWith(
      "Rate limit reached. Please wait.",
    );
  });
});

describe("OpenAI Codex Subscription Provider", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("injects Authorization, chatgpt-account-id from extra, and OpenAI-Beta headers", async () => {
    let capturedUrl = "";
    let capturedInit: RequestInit | undefined;

    const fakeFetch: typeof fetch = async (url, init) => {
      capturedUrl = String(url);
      capturedInit = init;
      return new Response(JSON.stringify({ id: "resp_123" }), {
        status: 200,
        headers: { "Content-Type": "application/json" },
      });
    };

    const codexFetch = createOpenAICodexSubscriptionFetch({
      fetchFn: fakeFetch,
      getToken: async () => ({
        token: "openai-oauth-token-123",
        extra: {
          chatgpt_account_id: "acct-test-456",
        },
      }),
      catalog: {
        id: "openai_codex",
        enabled: true,
        label: "OpenAI Codex",
        beta_headers: {
          "OpenAI-Beta": "responses=experimental",
        },
        system_prefix: null,
        base_url: "https://chatgpt.com/backend-api/codex",
        notes: "",
      },
    });

    await codexFetch("https://chatgpt.com/backend-api/codex/responses", {
      method: "POST",
      body: JSON.stringify({ model: "gpt-6.1-sol", input: "test" }),
      headers: { "Content-Type": "application/json" },
    });

    expect(capturedUrl).toBe("https://chatgpt.com/backend-api/codex/responses");
    const headers = new Headers(capturedInit?.headers);
    expect(headers.get("Authorization")).toBe("Bearer openai-oauth-token-123");
    expect(headers.get("chatgpt-account-id")).toBe("acct-test-456");
    expect(headers.get("OpenAI-Beta")).toBe("responses=experimental");
  });
});

function sse(...events: Array<Record<string, unknown>>): string {
  return events
    .map((e) => `event: ${String(e.type)}\ndata: ${JSON.stringify(e)}\n\n`)
    .join("");
}

const finalResponse = {
  id: "resp_0a1b",
  object: "response",
  status: "completed",
  model: "gpt-6.1-sol",
  output: [
    {
      id: "msg_0a1b",
      type: "message",
      status: "completed",
      role: "assistant",
      content: [{ type: "output_text", text: "Hello there", annotations: [] }],
    },
  ],
  usage: { input_tokens: 9, output_tokens: 3, total_tokens: 12 },
};

const helloStream = (completedOutput: unknown[]) =>
  sse(
    {
      type: "response.created",
      response: { id: "resp_0a1b", object: "response", status: "in_progress" },
    },
    {
      type: "response.output_item.added",
      output_index: 0,
      item: { id: "msg_0a1b", type: "message", role: "assistant", content: [] },
    },
    {
      type: "response.output_text.delta",
      output_index: 0,
      content_index: 0,
      delta: "Hello",
    },
    {
      type: "response.output_text.delta",
      output_index: 0,
      content_index: 0,
      delta: " there",
    },
    {
      type: "response.output_item.done",
      output_index: 0,
      item: finalResponse.output[0],
    },
    {
      type: "response.completed",
      response: { ...finalResponse, output: completedOutput },
    },
  );

describe("OpenAI Codex request/response shaping", () => {
  it("forces store:false and stream:true while keeping the rest of the body", () => {
    const out = JSON.parse(
      transformCodexRequestBody(
        JSON.stringify({
          model: "gpt-6.1-sol",
          input: [{ role: "user", content: "hi" }],
          store: true,
          stream: false,
        }),
      ),
    );
    expect(out).toEqual({
      model: "gpt-6.1-sol",
      input: [{ role: "user", content: "hi" }],
      store: false,
      stream: true,
    });

    expect(JSON.parse(transformCodexRequestBody("{}"))).toEqual({
      store: false,
      stream: true,
    });
  });

  it("leaves a non-JSON body untouched", () => {
    expect(transformCodexRequestBody("not json")).toBe("not json");
  });

  it("reassembles response.completed into the non-streaming JSON", () => {
    const { status, body } = reassembleCodexResponse(
      helloStream(finalResponse.output),
    );
    expect(status).toBe(200);
    expect(body).toEqual(finalResponse);
  });

  it("fills an empty response.completed output from the streamed items", () => {
    const { body } = reassembleCodexResponse(helloStream([]));
    expect(body).toEqual(finalResponse);
  });

  it("falls back to concatenated text deltas when no terminal event arrived", () => {
    const text = helloStream([])
      .split("\n\n")
      .filter((b) => !b.includes("response.completed"))
      .filter((b) => !b.includes("response.output_item.done"))
      .join("\n\n");
    const { status, body } = reassembleCodexResponse(text) as {
      status: number;
      body: {
        status: string;
        output: Array<{ content: Array<{ text: string }> }>;
      };
    };
    expect(status).toBe(200);
    expect(body.status).toBe("completed");
    expect(body.output[0].content[0].text).toBe("Hello there");
  });

  it("maps a stream error event to an error payload", () => {
    const { status, body } = reassembleCodexResponse(
      sse({ type: "error", code: "rate_limit", message: "slow down" }),
    );
    expect(status).toBe(502);
    expect(body).toEqual({
      error: { message: "slow down", code: "rate_limit" },
    });
  });

  it("passes the SSE through untouched when the caller asked for streaming", async () => {
    const raw = helloStream(finalResponse.output);
    const response = new Response(raw, {
      status: 200,
      headers: { "content-type": "text/event-stream" },
    });
    const out = await transformCodexResponse(response, {
      requestBody: JSON.stringify({ model: "gpt-6.1-sol", stream: true }),
    });
    expect(out).toBe(response);
    expect(await out.text()).toBe(raw);
  });

  it("does not touch error responses", async () => {
    const response = new Response('{"error":{"message":"nope"}}', {
      status: 400,
      headers: { "content-type": "application/json" },
    });
    const out = await transformCodexResponse(response, {
      requestBody: JSON.stringify({ model: "x" }),
    });
    expect(out).toBe(response);
  });

  it("generateText-style call: backend gets a streaming, non-stored request and the caller gets plain JSON", async () => {
    let sentBody: Record<string, unknown> = {};
    const codexFetch = createOpenAICodexSubscriptionFetch({
      fetchFn: async (_url, init) => {
        sentBody = JSON.parse(String(init?.body));
        return new Response(helloStream(finalResponse.output), {
          status: 200,
          headers: {
            "content-type": "text/event-stream",
            "content-length": "999",
          },
        });
      },
      getToken: async () => ({ token: "tok", extra: {} }),
    });

    const res = await codexFetch(
      "https://chatgpt.com/backend-api/codex/responses",
      {
        method: "POST",
        body: JSON.stringify({ model: "gpt-6.1-sol", input: "hi" }),
      },
    );

    expect(sentBody.store).toBe(false);
    expect(sentBody.stream).toBe(true);
    expect(res.headers.get("content-type")).toBe("application/json");
    expect(await res.json()).toEqual(finalResponse);
  });

  it("reassembles even when the backend sends no content-type (real Codex behavior without Accept)", async () => {
    let sentHeaders = new Headers();
    const codexFetch = createOpenAICodexSubscriptionFetch({
      fetchFn: async (_url, init) => {
        sentHeaders = new Headers(init?.headers);
        // Measured 2026-11-03: chatgpt.com/backend-api/codex/responses
        // returns the SSE body with no content-type header at all.
        return new Response(helloStream(finalResponse.output), { status: 200 });
      },
      getToken: async () => ({ token: "tok", extra: {} }),
    });

    const res = await codexFetch(
      "https://chatgpt.com/backend-api/codex/responses",
      {
        method: "POST",
        body: JSON.stringify({ model: "gpt-6.1-sol", input: "hi" }),
      },
    );

    expect(sentHeaders.get("accept")).toBe("text/event-stream");
    expect(await res.json()).toEqual(finalResponse);
  });

  it("streamText-style call keeps the event stream", async () => {
    const raw = helloStream(finalResponse.output);
    const codexFetch = createOpenAICodexSubscriptionFetch({
      fetchFn: async () =>
        new Response(raw, {
          status: 200,
          headers: { "content-type": "text/event-stream" },
        }),
      getToken: async () => ({ token: "tok", extra: {} }),
    });

    const res = await codexFetch(
      "https://chatgpt.com/backend-api/codex/responses",
      {
        method: "POST",
        body: JSON.stringify({
          model: "gpt-6.1-sol",
          input: "hi",
          stream: true,
        }),
      },
    );
    expect(await res.text()).toBe(raw);
  });
});

describe("subscriptionModelIds", () => {
  const provider = (
    models: Array<{ id: string; label: string }> | undefined,
    default_model?: string | null,
  ) => ({
    id: "anthropic" as const,
    enabled: true,
    label: "x",
    beta_headers: {},
    system_prefix: null,
    base_url: "",
    notes: "",
    models,
    default_model,
  });

  it("prefers the catalog list and puts the default model first", () => {
    expect(
      subscriptionModelIds(
        provider(
          [
            { id: "a", label: "A" },
            { id: "b", label: "B" },
            { id: "c", label: "C" },
          ],
          "b",
        ),
        ["fallback-1"],
      ),
    ).toEqual(["b", "a", "c"]);
  });

  it("keeps catalog order when the default is absent or unknown", () => {
    const models = [
      { id: "a", label: "A" },
      { id: "b", label: "B" },
    ];
    expect(subscriptionModelIds(provider(models, null), ["f"])).toEqual([
      "a",
      "b",
    ]);
    expect(subscriptionModelIds(provider(models, "zzz"), ["f"])).toEqual([
      "a",
      "b",
    ]);
  });

  it("falls back to the hard-coded list for a catalog without models", () => {
    expect(subscriptionModelIds(provider(undefined), ["f1", "f2"])).toEqual([
      "f1",
      "f2",
    ]);
    expect(subscriptionModelIds(provider([]), ["f1"])).toEqual(["f1"]);
    expect(subscriptionModelIds(undefined, ["f1"])).toEqual(["f1"]);
  });
});

describe("Gemini Subscription Provider", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("rewrites generativelanguage URL to cloudcode-pa.googleapis.com", () => {
    const streamUrl =
      "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-pro:streamGenerateContent?alt=sse";
    const rewritten = rewriteGeminiUrl(streamUrl);
    expect(rewritten).toBe(
      "https://cloudcode-pa.googleapis.com/v1internal:streamGenerateContent?alt=sse",
    );

    const generateUrl =
      "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-flash:generateContent";
    const rewritten2 = rewriteGeminiUrl(generateUrl);
    expect(rewritten2).toBe(
      "https://cloudcode-pa.googleapis.com/v1internal:generateContent",
    );
  });

  it("wraps request body as { project, model, request: {...} }", () => {
    const rawBody = JSON.stringify({
      contents: [{ role: "user", parts: [{ text: "Hello Gemini" }] }],
    });

    const wrapped = JSON.parse(
      wrapGeminiRequestBody(rawBody, "gemini-2.5-pro", "my-gcp-project-123"),
    );

    expect(wrapped).toEqual({
      model: "gemini-2.5-pro",
      project: "my-gcp-project-123",
      request: {
        contents: [{ role: "user", parts: [{ text: "Hello Gemini" }] }],
      },
    });
  });

  it("unwraps JSON response from { response: VertexGenerateContentResponse }", async () => {
    const wrappedResponse = new Response(
      JSON.stringify({
        response: {
          candidates: [{ content: { parts: [{ text: "Hi from Gemini" }] } }],
        },
        traceId: "trace-xyz",
      }),
      {
        status: 200,
        headers: { "Content-Type": "application/json" },
      },
    );

    const unwrapped = await transformGeminiResponse(wrappedResponse);
    const data = await unwrapped.json();
    expect(data).toEqual({
      candidates: [{ content: { parts: [{ text: "Hi from Gemini" }] } }],
    });
  });

  it("unwraps SSE stream response data events from { response: {...} }", async () => {
    const sseChunk =
      'data: {"response":{"candidates":[{"content":{"parts":[{"text":"chunk 1"}]}}]}}\n\n';
    const sseResponse = new Response(
      new ReadableStream({
        start(controller) {
          controller.enqueue(new TextEncoder().encode(sseChunk));
          controller.close();
        },
      }),
      {
        status: 200,
        headers: { "Content-Type": "text/event-stream" },
      },
    );

    const transformed = await transformGeminiResponse(sseResponse);
    const text = await transformed.text();
    expect(text).toContain(
      'data: {"candidates":[{"content":{"parts":[{"text":"chunk 1"}]}}]}',
    );
  });

  it("injects Authorization header and wraps body in custom fetch", async () => {
    let capturedUrl = "";
    let capturedInit: RequestInit | undefined;

    const fakeFetch: typeof fetch = async (url, init) => {
      capturedUrl = String(url);
      capturedInit = init;
      return new Response(
        JSON.stringify({
          response: { candidates: [] },
        }),
        {
          status: 200,
          headers: { "Content-Type": "application/json" },
        },
      );
    };

    const geminiFetch = createGeminiSubscriptionFetch("gemini-2.5-pro", {
      fetchFn: fakeFetch,
      getToken: async () => ({
        token: "google-oauth-token-xyz",
        extra: { project_id: "test-proj-456" },
      }),
    });

    await geminiFetch(
      "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-pro:streamGenerateContent?alt=sse",
      {
        method: "POST",
        body: JSON.stringify({ contents: [] }),
        headers: { "Content-Type": "application/json" },
      },
    );

    expect(capturedUrl).toBe(
      "https://cloudcode-pa.googleapis.com/v1internal:streamGenerateContent?alt=sse",
    );
    const headers = new Headers(capturedInit?.headers);
    expect(headers.get("Authorization")).toBe("Bearer google-oauth-token-xyz");
    const parsedBody = JSON.parse(String(capturedInit?.body));
    expect(parsedBody.model).toBe("gemini-2.5-pro");
    expect(parsedBody.project).toBe("test-proj-456");
    expect(parsedBody.request).toEqual({ contents: [] });
  });

  it("strips content-length and content-encoding in transformGeminiResponse", async () => {
    const initialHeaders = new Headers({
      "Content-Type": "application/json",
      "Content-Length": "1234",
      "Content-Encoding": "gzip",
    });
    const jsonResponse = new Response(
      JSON.stringify({
        response: { candidates: [] },
        traceId: "test-trace",
      }),
      {
        status: 200,
        headers: initialHeaders,
      },
    );

    const transformed = await transformGeminiResponse(jsonResponse);
    expect(transformed.headers.get("content-length")).toBeNull();
    expect(transformed.headers.get("content-encoding")).toBeNull();

    const sseHeaders = new Headers({
      "Content-Type": "text/event-stream",
      "Content-Length": "5678",
      "Content-Encoding": "deflate",
    });
    const sseResponse = new Response(
      new ReadableStream({
        start(controller) {
          controller.enqueue(new TextEncoder().encode("data: [DONE]\n\n"));
          controller.close();
        },
      }),
      {
        status: 200,
        headers: sseHeaders,
      },
    );

    const transformedSse = await transformGeminiResponse(sseResponse);
    expect(transformedSse.headers.get("content-length")).toBeNull();
    expect(transformedSse.headers.get("content-encoding")).toBeNull();
  });

  it("re-reads project_id from token on each request and on 401 retry", async () => {
    let attempt = 0;
    const capturedBodies: Array<Record<string, unknown>> = [];

    const fakeFetch: typeof fetch = async (_url, init) => {
      attempt++;
      capturedBodies.push(JSON.parse(String(init?.body)));
      if (attempt === 1) {
        return new Response(
          JSON.stringify({ error: { message: "stale token" } }),
          {
            status: 401,
            headers: { "Content-Type": "application/json" },
          },
        );
      }
      return new Response(JSON.stringify({ response: { candidates: [] } }), {
        status: 200,
        headers: { "Content-Type": "application/json" },
      });
    };

    let tokenCall = 0;
    const geminiFetch = createGeminiSubscriptionFetch("gemini-2.5-pro", {
      fetchFn: fakeFetch,
      getToken: async () => {
        tokenCall++;
        return {
          token: `token-${tokenCall}`,
          extra: { project_id: `proj-${tokenCall}` },
        };
      },
    });

    const res = await geminiFetch(
      "https://cloudcode-pa.googleapis.com/v1internal:generateContent",
      {
        method: "POST",
        body: JSON.stringify({ contents: [{ text: "hi" }] }),
        headers: { "Content-Type": "application/json" },
      },
    );

    expect(res.status).toBe(200);
    expect(attempt).toBe(2);
    expect(capturedBodies[0].project).toBe("proj-1");
    expect(capturedBodies[1].project).toBe("proj-2");
  });

  it("creates GoogleGenerativeAI model bound to custom subscription fetch", () => {
    const model = createGeminiSubscriptionModel("gemini-2.5-pro", {
      getToken: async () => ({
        token: "test-token",
        extra: { project_id: "test-project" },
      }),
    });
    expect(model).toBeDefined();
    expect(model.modelId).toBe("gemini-2.5-pro");
    expect(model.provider).toBe("google.generative-ai");
  });
});

describe("Subscription Fetch 401 Retry & Error Handling", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("surfaces original vendor error when 401 retry also fails", async () => {
    let attempt = 0;
    const fakeFetch: typeof fetch = async () => {
      attempt++;
      if (attempt === 1) {
        return new Response(
          JSON.stringify({
            error: { message: "Original token expired or invalid" },
          }),
          { status: 401, headers: { "Content-Type": "application/json" } },
        );
      }
      return new Response(
        JSON.stringify({
          error: { message: "Retry refresh failed: token revoked" },
        }),
        { status: 401, headers: { "Content-Type": "application/json" } },
      );
    };

    let tokenCalls = 0;
    const customFetch = createSubscriptionFetch({
      provider: "anthropic",
      fetchFn: fakeFetch,
      getToken: async () => {
        tokenCalls++;
        return { token: `token-${tokenCalls}`, extra: {} };
      },
    });

    const res = await customFetch("https://api.anthropic.com/v1/messages", {
      method: "POST",
      body: JSON.stringify({ messages: [] }),
    });

    expect(res.status).toBe(401);
    expect(attempt).toBe(2);
    expect(tokenCalls).toBe(2);
    expect(mocks.sonnerToastError).toHaveBeenCalledWith(
      "Original token expired or invalid. Please sign in again.",
    );
  });

  it("buffers ReadableStream request body before first attempt and reuses it on retry", async () => {
    let attempt = 0;
    const capturedBodies: string[] = [];

    const fakeFetch: typeof fetch = async (_url, init) => {
      attempt++;
      capturedBodies.push(String(init?.body));
      if (attempt === 1) {
        return new Response(JSON.stringify({ error: "401 Unauthorized" }), {
          status: 401,
          headers: { "Content-Type": "application/json" },
        });
      }
      return new Response(JSON.stringify({ ok: true }), {
        status: 200,
        headers: { "Content-Type": "application/json" },
      });
    };

    const payloadText = JSON.stringify({
      prompt: "streamed message to buffer",
    });
    const stream = new ReadableStream({
      start(controller) {
        controller.enqueue(new TextEncoder().encode(payloadText));
        controller.close();
      },
    });

    const customFetch = createSubscriptionFetch({
      provider: "anthropic",
      fetchFn: fakeFetch,
      getToken: async () => ({ token: "dummy-token", extra: {} }),
    });

    const res = await customFetch("https://api.anthropic.com/v1/messages", {
      method: "POST",
      body: stream,
    });

    expect(res.status).toBe(200);
    expect(attempt).toBe(2);
    expect(capturedBodies[0]).toBe(payloadText);
    expect(capturedBodies[1]).toBe(payloadText);
  });

  it("runs transformResponse on every returned response path", async () => {
    const transformTracker = vi.fn((resp: Response) => {
      const headers = new Headers(resp.headers);
      headers.set("X-Transformed", "true");
      return new Response(resp.body, {
        status: resp.status,
        statusText: resp.statusText,
        headers,
      });
    });

    // Path 1: 200 OK
    const fetch200 = createSubscriptionFetch({
      provider: "anthropic",
      fetchFn: async () => new Response("ok", { status: 200 }),
      getToken: async () => ({ token: "tok", extra: {} }),
      transformResponse: transformTracker,
    });
    const res200 = await fetch200("https://api.anthropic.com/v1/messages");
    expect(res200.status).toBe(200);
    expect(res200.headers.get("X-Transformed")).toBe("true");

    // Path 2: 403 Forbidden
    const fetch403 = createSubscriptionFetch({
      provider: "anthropic",
      fetchFn: async () => new Response("forbidden", { status: 403 }),
      getToken: async () => ({ token: "tok", extra: {} }),
      transformResponse: transformTracker,
    });
    const res403 = await fetch403("https://api.anthropic.com/v1/messages");
    expect(res403.status).toBe(403);
    expect(res403.headers.get("X-Transformed")).toBe("true");

    // Path 3: 401 retry failure
    const fetch401Fail = createSubscriptionFetch({
      provider: "anthropic",
      fetchFn: async () => new Response("unauthorized", { status: 401 }),
      getToken: async () => ({ token: "tok", extra: {} }),
      transformResponse: transformTracker,
    });
    const res401 = await fetch401Fail("https://api.anthropic.com/v1/messages");
    expect(res401.status).toBe(401);
    expect(res401.headers.get("X-Transformed")).toBe("true");
  });

  it("extractVendorErrorMessage truncates long error messages with ellipsis", async () => {
    const longText = "A".repeat(350);
    const longResponse = new Response(longText, {
      status: 500,
      statusText: "Server Error",
      headers: { "Content-Type": "text/plain" },
    });

    const msg = await extractVendorErrorMessage(longResponse);
    expect(msg.length).toBe(303);
    expect(msg.endsWith("...")).toBe(true);
    expect(msg.startsWith("A".repeat(300))).toBe(true);
  });
});

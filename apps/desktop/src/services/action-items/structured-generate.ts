/**
 * Structured generation that actually works on local models (WS-C, PG gate).
 *
 * The AI-SDK `@ai-sdk/openai-compatible` provider drives ollama via
 * `/v1/chat/completions` with `response_format: json_schema`. On reasoning
 * models (qwen3, etc.) this returns prose / empty content because the model's
 * `<think>` stream isn't grammar-constrained — `generateObject` then throws
 * `AI_NoObjectGeneratedError` (see the eval README).
 *
 * ollama's NATIVE `/api/chat` with `format: <json-schema>` + `think:false`
 * grammar-constrains the decode, so the model can only emit the object. This
 * helper routes ollama structured calls there and falls back to the AI-SDK
 * `generateObject` for every other provider.
 */

import { fetch as tauriFetch } from "@tauri-apps/plugin-http";
import { generateObject, type LanguageModel } from "ai";
import type { z } from "zod";

export type StructuredTarget = {
  providerId: string;
  modelId: string;
  /** Provider base URL (may end in /v1). */
  baseUrl: string;
};

export type StructuredDeps = {
  /** Injected for tests (AI-SDK path). */
  generateObjectFn?: typeof generateObject;
  /** Injected for tests (ollama-native path). */
  fetchFn?: typeof fetch;
};

/**
 * Generate an object matching `schema`. For ollama, uses the native `format`
 * endpoint so reasoning models produce valid JSON; otherwise the AI-SDK path.
 */
export async function generateStructured<T>(
  model: LanguageModel,
  args: { schema: z.ZodType<T>; prompt: string },
  target: StructuredTarget,
  deps: StructuredDeps = {},
): Promise<T> {
  if (target.providerId === "ollama") {
    return generateViaOllamaFormat(
      args.schema,
      args.prompt,
      target,
      deps.fetchFn,
    );
  }
  const gen = deps.generateObjectFn ?? generateObject;
  const result = await gen({ model, schema: args.schema, prompt: args.prompt });
  return result.object as T;
}

async function generateViaOllamaFormat<T>(
  schema: z.ZodType<T>,
  prompt: string,
  target: StructuredTarget,
  fetchFn?: typeof fetch,
): Promise<T> {
  // zod v4 -> JSON schema for ollama's grammar-constrained `format`.
  const jsonSchema = (schema as unknown as { toJSONSchema?: () => unknown })
    .toJSONSchema
    ? (schema as unknown as { toJSONSchema: () => unknown }).toJSONSchema()
    : zToJsonSchema(schema);

  const apiUrl = `${target.baseUrl.replace(/\/v1\/?$/, "")}/api/chat`;
  const doFetch = fetchFn ?? (tauriFetch as unknown as typeof fetch);

  const resp = await doFetch(apiUrl, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      model: target.modelId,
      messages: [{ role: "user", content: prompt }],
      format: jsonSchema,
      // Reasoning models must not emit <think> outside the constrained object.
      think: false,
      stream: false,
      options: { num_predict: 2048, temperature: 0 },
    }),
  });

  if (!resp.ok) {
    throw new Error(`ollama /api/chat ${resp.status}: ${await safeText(resp)}`);
  }
  const body = (await resp.json()) as { message?: { content?: string } };
  const content = body.message?.content ?? "";
  let parsed: unknown;
  try {
    parsed = JSON.parse(content);
  } catch {
    throw new Error(
      `ollama returned non-JSON content: ${content.slice(0, 200)}`,
    );
  }
  // Validate against the schema so downstream sees the same shape as the AI-SDK path.
  return schema.parse(parsed);
}

// zod v4 exposes z.toJSONSchema; guard for older builds.
function zToJsonSchema(schema: z.ZodType): unknown {
  // eslint-disable-next-line @typescript-eslint/no-var-requires
  const { z } = require("zod") as typeof import("zod");
  return z.toJSONSchema(schema);
}

async function safeText(resp: Response): Promise<string> {
  try {
    return (await resp.text()).slice(0, 200);
  } catch {
    return "<no body>";
  }
}

import { createOpenAI } from "@ai-sdk/openai";
import { customProvider } from "ai";

import { isTauri } from "@tauri-apps/api/core";
import { fetch as tauriFetch } from "@tauri-apps/plugin-http";

import { commands as sseCommands } from "@hypr/plugin-sse";
import { commands as authCommands } from "@hypr/plugin-auth";
import { commands as connectorCommands } from "@hypr/plugin-connector";

export const fetch = (
  input: Parameters<typeof globalThis.fetch>[0],
  init?: Parameters<typeof globalThis.fetch>[1],
) => {
  if (!isTauri()) {
    return globalThis.fetch(input, init);
  }

  const headers =
    init?.headers instanceof Headers ? Array.from(init.headers.entries()) : [];

  const isSSE = headers.some(
    ([key, value]) =>
      key.toLowerCase() === "accept" &&
      value.toLowerCase() === "text/event-stream",
  );

  const f = isSSE ? sseCommands.fetch : tauriFetch;
  return f(input, init);
};

const getModel = async (model: string) => {
  const apiBase = await connectorCommands.getApiBase("auto-llm");
  const apiKey = await authCommands.getFromVault("remote-server");

  if (!apiBase) {
    throw new Error("no_api_base");
  }

  const openai = createOpenAI({
    baseURL: apiBase,
    apiKey: apiKey ?? undefined,
    fetch,
  });

  return openai(model, { structuredOutputs: true });
};

export const modelProvider = async () => {
  const any = await getModel("gpt-4");

  return customProvider({
    languageModels: { any },
  });
};

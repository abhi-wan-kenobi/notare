import { createOpenAI } from "@ai-sdk/openai";
import { customProvider } from "ai";

import { fetch as tauriFetch } from "@tauri-apps/plugin-http";
import { commands as sseCommands } from "@hypr/plugin-sse";
import { commands as connectorCommands } from "@hypr/plugin-connector";

import {
  WidgetOneByOne,
  WidgetTwoByOne,
  WidgetTwoByTwo,
  WidgetFullSizeModal,
} from "@hypr/ui/components/ui/widgets";

export * from "ai";

export const fetch = (
  input: Parameters<typeof globalThis.fetch>[0],
  init?: Parameters<typeof globalThis.fetch>[1],
) => {
  // @ts-ignore
  const isTauri = !!window.__TAURI__;

  if (!isTauri) {
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

  const openai = createOpenAI({
    baseURL: apiBase ?? "http://localhost:1234/v1",
    apiKey: "NOT_NEEDED",
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

export interface Extension {
  [key: string]: Widget[];
}

export interface Widget {
  id: string;
  init: () => Promise<void>;
  component:
    | typeof WidgetOneByOne
    | typeof WidgetTwoByOne
    | typeof WidgetTwoByTwo
    | typeof WidgetFullSizeModal;
}

export { formatTime } from "./time";

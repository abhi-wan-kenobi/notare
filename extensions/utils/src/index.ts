import { createOpenAI } from "@ai-sdk/openai";
import { customProvider } from "ai";

import { fetch as tauriFetch } from "@tauri-apps/plugin-http";
import { commands as sseCommands } from "@hypr/plugin-sse";
import { commands as connectorCommands } from "@hypr/plugin-connector";

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
  const apiBase = await connectorCommands.getApiBase("LocalLlm");

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

/* 
NOTE: 
Extensions are modular plugins that enhance Hyprnote’s functionality.
Before creating an extension, we recommend reviewing the documentation.
Extensions integrate as widgets of varying sizes, with the default size set to twoByTwo.
*/
export interface Extension {
  init: () => Promise<void>;
  oneByOne?: (props: { onMaximize?: () => void }) => React.ReactNode;
  twoByOne?: (props: { onMaximize?: () => void }) => React.ReactNode;
  twoByTwo: (props: { onMaximize?: () => void }) => React.ReactNode;
  full?: (props: { onMinimize: () => void }) => React.ReactNode;
}

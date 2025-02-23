import { fetch as tauriFetch } from "@tauri-apps/plugin-http";
import { createOpenAI } from "@ai-sdk/openai";
import { customProvider } from "ai";

export * from "ai";

// @ts-ignore
export const fetch = window.__TAURI__ ? tauriFetch : globalThis.fetch;

const openai = createOpenAI({
  baseURL: "http://localhost:1234/v1",
  apiKey: "NOT_NEEDED",
  fetch,
});

export const modelProvider = customProvider({
  languageModels: {
    any: openai("gpt-4", { structuredOutputs: true }),
  },
});

export interface Extension {
  init: () => Promise<void>;
  modal?: (props: { onClose: () => void }) => React.ReactNode;
  panelTwoByTwo?: (props: {}) => React.ReactNode;
  panelFull?: (props: {}) => React.ReactNode;
}

import type { Meta, StoryObj } from "@storybook/react";

import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { http, HttpResponse } from "msw";
import { mockIPC } from "@tauri-apps/api/mocks";

import { ChatCompletion } from "openai/resources/chat/completions";
import { createConfig, createClient } from "@hypr/client";

import type { LiveSummaryResponse } from "./types";
import extension from "./index";

const queryClient = new QueryClient();

const client = createClient(
  createConfig({
    fetch,
    auth: () => "123",
    baseUrl: "http://localhost:1234",
  }),
);

const meta = {
  title: "Extensions/Live Summary",
  component: extension.modal,
} satisfies Meta<typeof extension.modal>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Main: Story = {
  parameters: {
    msw: {
      handlers: [
        http.post(
          "http://localhost:1234/v1/chat/completions",
          async ({ request }) => {
            const body = await request.json();
            console.log("body", body);

            const data = {
              points: ["first point", "second point", "third point"],
            } satisfies LiveSummaryResponse;

            const res: ChatCompletion = {
              id: "123",
              object: "chat.completion",
              created: 123,
              model: "gpt-4o-mini",
              choices: [
                {
                  message: {
                    content: JSON.stringify(data),
                    role: "assistant",
                    refusal: null,
                  },
                  finish_reason: "stop",
                  index: 0,
                  logprobs: null,
                },
              ],
            };

            return HttpResponse.json(res);
          },
        ),
      ],
    },
  },
  decorators: [
    (Story) => {
      mockIPC((cmd, args: any) => {
        if (cmd == "plugin:template|render") {
          if (args.name.includes("system")) {
            return "You are a helpful assistant.";
          }

          if (args.name.includes("user")) {
            return "generate a random json object. Each contains a one fun sentence.";
          }
        }
        return {};
      });

      return (
        <QueryClientProvider client={queryClient}>
          {Story()}
        </QueryClientProvider>
      );
    },
  ],
  args: {
    client,
    onClose: () => {},
  },
};

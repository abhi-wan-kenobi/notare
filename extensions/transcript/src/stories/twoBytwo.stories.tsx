import type { Meta, StoryObj } from "@storybook/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { http, HttpResponse, type HttpHandler } from "msw";
import { mockIPC } from "@tauri-apps/api/mocks";
import { mockTimeline } from "./mockData";

import extension from "../index";

const queryClient = new QueryClient();

const meta = {
  title: "Extensions/Transcript/2x2",
  component: extension.twoByTwo,
} satisfies Meta<typeof extension.twoByTwo>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Main: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("http://localhost:1234/api/timeline", () => {
          return HttpResponse.json({
            timeline: mockTimeline,
          });
        }),
      ] satisfies HttpHandler[],
    },
  },
  decorators: [
    (Story: any) => {
      mockIPC((cmd) => {
        if (cmd === "plugin:connector|get_api_base") {
          return "http://localhost:1234/v1";
        }

        if (cmd === "plugin:sse|fetch" || cmd === "plugin:http|fetch") {
          return {
            timeline: mockTimeline,
          };
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
    onMaximize: () => {},
    children: (
      <>
        <div>Example Widget Content</div>
      </>
    ),
  },
};

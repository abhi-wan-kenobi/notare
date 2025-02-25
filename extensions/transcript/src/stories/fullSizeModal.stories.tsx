import type { Meta, StoryObj } from "@storybook/react";
import { http, HttpResponse, type HttpHandler } from "msw";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { mockIPC } from "@tauri-apps/api/mocks";
import { mockTimeline } from "./mockData";

import { Channel } from "@tauri-apps/api/core";
import { type SessionEvent } from "@hypr/plugin-listener";

import extension from "../index";

const queryClient = new QueryClient();

const meta = {
  title: "Extensions/Transcript/Full Size Modal",
  component: extension.full,
} satisfies Meta<typeof extension.full>;

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
    (story: any) => {
      mockIPC((cmd, args) => {
        if (cmd == "plugin:listener|subscribe") {
          const channel: Channel<SessionEvent> = (args as any).channel;

          const sleep = (ms: number) =>
            new Promise((resolve) => setTimeout(resolve, ms));

          (async () => {
            await sleep(400);
            channel.onmessage({
              type: "timelineView",
              timeline: {
                items: [
                  {
                    start: 0,
                    end: 100,
                    speaker: "Speaker 1",
                    text: "Hello, world!",
                  },
                  {
                    start: 100,
                    end: 200,
                    speaker: "Speaker 2",
                    text: "Hi, world!",
                  },
                ],
              },
            });
            await sleep(800);
            channel.onmessage({
              type: "timelineView",
              timeline: {
                items: [
                  {
                    start: 200,
                    end: 300,
                    speaker: "Speaker 1",
                    text: "Hello, world!",
                  },
                  {
                    start: 300,
                    end: 400,
                    speaker: "Speaker 2",
                    text: "Hi, world!",
                  },
                ],
              },
            });
            await sleep(300);
          })();
        }

        return {};
      });

      return (
        <QueryClientProvider client={queryClient}>
          {story()}
        </QueryClientProvider>
      );
    },
  ],
  args: {
    onMinimize: () => {},
    children: (
      <>
        <div>Example Widget Content</div>
      </>
    ),
  },
};

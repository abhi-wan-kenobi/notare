import type { Meta, StoryObj } from "@storybook/react";
import { http, HttpResponse, type HttpHandler } from "msw";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { mockIPC } from "@tauri-apps/api/mocks";
import { mockTimeline } from "./mockData";

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
      mockIPC((_cmd, _args) => {
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

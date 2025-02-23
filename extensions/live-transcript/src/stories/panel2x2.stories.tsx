import type { Meta, StoryObj } from "@storybook/react";
import type { HttpHandler } from "msw";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { mockIPC } from "@tauri-apps/api/mocks";

import extension from "../index";

const queryClient = new QueryClient();

const meta = {
  title: "Extensions/Live Transcript/Panel 2x2",
  component: extension.panelTwoByTwo,
} satisfies Meta<typeof extension.panelTwoByTwo>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Main2: Story = {
  parameters: {
    msw: {
      handlers: [] satisfies HttpHandler[],
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
  args: {},
};

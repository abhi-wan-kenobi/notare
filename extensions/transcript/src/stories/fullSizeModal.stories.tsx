import type { Meta, StoryObj } from "@storybook/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { mockTranscriptIPC } from "./mocks";

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
      handlers: [],
    },
  },
  decorators: [
    (Story: any) => {
      mockTranscriptIPC();

      return (
        <QueryClientProvider client={queryClient}>
          <div style={{ height: "80vh" }}>{Story()}</div>
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

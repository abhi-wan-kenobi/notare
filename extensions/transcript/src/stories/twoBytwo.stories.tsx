import type { Meta, StoryObj } from "@storybook/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { mockTranscriptIPC } from "./mocks";

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
      handlers: [],
    },
  },
  decorators: [
    (Story: any) => {
      mockTranscriptIPC();

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

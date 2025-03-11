import type { Meta, StoryObj } from "@storybook/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import LiveTranscript2x2 from "../widgets/default/2x2";
import { mockTranscriptIPC } from "./mocks";

const queryClient = new QueryClient();

const meta = {
  title: "Transcript/Default/2x2",
  component: LiveTranscript2x2,
} satisfies Meta<typeof LiveTranscript2x2>;

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
  },
};

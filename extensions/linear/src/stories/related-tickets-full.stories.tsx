import type { Meta, StoryObj } from "@storybook/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";

import RelatedTicketsFull from "../widgets/related-tickets/full";

const queryClient = new QueryClient();

const meta = {
  title: "Linear/Related Tickets/Full",
  component: RelatedTicketsFull,
} satisfies Meta<typeof RelatedTicketsFull>;

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
      return (
        <QueryClientProvider client={queryClient}>
          <div style={{ height: "80vh" }}>{Story()}</div>
        </QueryClientProvider>
      );
    },
  ],
  args: {
    onMinimize: () => {},
  },
};

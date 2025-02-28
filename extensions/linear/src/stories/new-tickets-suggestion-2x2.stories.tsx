import type { Meta, StoryObj } from "@storybook/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import NewTicketsSuggestion2x2 from "../widgets/new-tickets-suggestion/2x2";

const queryClient = new QueryClient();

const meta = {
  title: " Linear/New Tickets Suggestion/2x2",
  component: NewTicketsSuggestion2x2,
} satisfies Meta<typeof NewTicketsSuggestion2x2>;

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
          {Story()}
        </QueryClientProvider>
      );
    },
  ],
  args: {
    onMaximize: () => {},
  },
};

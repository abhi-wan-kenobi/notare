import type { Meta, StoryObj } from "@storybook/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import NewTicketsSuggestionFull from "../widgets/new-tickets-suggestion/full";

const queryClient = new QueryClient();

const meta = {
  title: "Linear/New Tickets Suggestion/Full",
  component: NewTicketsSuggestionFull,
} satisfies Meta<typeof NewTicketsSuggestionFull>;

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

import type { Meta, StoryObj } from "@storybook/react";

import Twenty2x1 from "../widgets/default/2x1";
import MockProvider from "../widgets/default/mock";

const meta = {
  title: "Twenty/Default/2x1",
  component: Twenty2x1,
} satisfies Meta<typeof Twenty2x1>;

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
        <MockProvider>
          {Story()}
        </MockProvider>
      );
    },
  ],
};

import type { Meta, StoryObj } from "@storybook/react";
import Analog2x2 from "../widgets/analog/2x2";

const meta = {
  title: "Timer/Analog/2x2",
  component: Analog2x2,
} satisfies Meta<typeof Analog2x2>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Main: Story = {
  parameters: {
    msw: {
      handlers: [],
    },
  },
};

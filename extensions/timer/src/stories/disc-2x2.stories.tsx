import type { Meta, StoryObj } from "@storybook/react";
import AnalogDiscTimer2x2 from "../widgets/disc/2x2";

const meta = {
  title: "Timer/Disc/2x2",
  component: AnalogDiscTimer2x2,
} satisfies Meta<typeof AnalogDiscTimer2x2>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Main: Story = {
  parameters: {
    msw: {
      handlers: [],
    },
  },
};

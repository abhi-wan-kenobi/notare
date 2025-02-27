import type { Meta, StoryObj } from "@storybook/react";
import DigitalDigitsTimer2x1 from "../widgets/digits/2x1";

const meta = {
  title: "Timer/Digits/2x1",
  component: DigitalDigitsTimer2x1,
} satisfies Meta<typeof DigitalDigitsTimer2x1>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Main: Story = {
  parameters: {
    msw: {
      handlers: [],
    },
  },
};

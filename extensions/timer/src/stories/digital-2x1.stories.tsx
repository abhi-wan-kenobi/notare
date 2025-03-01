import type { Meta, StoryObj } from "@storybook/react";
import Digital2x1 from "../widgets/digital/2x1";

const meta = {
  title: "Timer/Digital/2x1",
  component: Digital2x1,
} satisfies Meta<typeof Digital2x1>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Main: Story = {
  parameters: {
    msw: {
      handlers: [],
    },
  },
};

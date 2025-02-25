import type { Meta, StoryObj } from "@storybook/react";

import component from "./index";

const meta = {
  title: "Extensions/World Clocks",
  component: component.twoByTwo,
} satisfies Meta<typeof component.twoByTwo>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Main: Story = {
  args: {
    children: <div>Hello</div>,
  },
};

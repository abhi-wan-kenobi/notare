import type { Meta, StoryObj } from "@storybook/react";

import component from "./index";

const meta = {
  title: "Extensions/World Clocks",
  component: component.panelTwoByTwo,
} satisfies Meta<typeof component.panelTwoByTwo>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Main: Story = {
  args: {},
};

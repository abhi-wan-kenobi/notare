import type { Meta, StoryObj } from "@storybook/react";

import extension from "../index";

const meta = {
  title: "Extensions/Dino Game/2x1",
  component: extension.twoByOne,
} satisfies Meta<typeof extension.twoByOne>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Main: Story = {
  parameters: {
    msw: {
      handlers: [],
    },
  },
  args: {
    onMaximize: () => {},
    children: (
      <>
        <div>Example Widget Content</div>
      </>
    ),
  },
};

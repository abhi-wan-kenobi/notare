import type { Meta, StoryObj } from "@storybook/react";

import extension from "../index";

const meta = {
  title: "Extensions/Timer/2x2",
  component: extension.twoByTwo,
} satisfies Meta<typeof extension.twoByTwo>;

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

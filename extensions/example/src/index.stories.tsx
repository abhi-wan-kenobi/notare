import type { Meta, StoryObj } from "@storybook/react";
import { mockIPC } from "@tauri-apps/api/mocks";

import extension from "./index";

const meta = {
  title: "Extensions/2x2",
  component: extension.twoByTwo,
} satisfies Meta<typeof extension.twoByTwo>;

export default meta;
type Story = StoryObj<typeof meta>;

export const TwoByTwo: Story = {
  parameters: {
    msw: {
      handlers: [],
    },
  },
  decorators: [
    (Story) => {
      mockIPC((_cmd, _args) => {
        return {};
      });

      return Story();
    },
  ],
  args: {
    children: "Example extension",
  },
};

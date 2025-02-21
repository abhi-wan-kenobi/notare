import type { Meta, StoryObj } from "@storybook/react";
import { mockIPC } from "@tauri-apps/api/mocks";

import extension from "./index";

const meta = {
  title: "Extensions/Example",
  component: extension.modal,
} satisfies Meta<typeof extension.modal>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Modal: Story = {
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
    onClose: () => {},
  },
};

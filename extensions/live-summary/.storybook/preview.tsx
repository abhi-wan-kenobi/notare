import type { Preview } from "@storybook/react";
import { initialize, mswLoader } from "msw-storybook-addon";
import "../src/globals.css";

initialize();

// @ts-ignore
window.STORYBOOK = true;

const preview: Preview = {
  loaders: [mswLoader],
};

export default preview;

import type { Config } from "tailwindcss";
import typography from "@tailwindcss/typography";

import Tiptap from "@hypr/tiptap/editor/tailwind.config";

import ExtensionLiveSummary from "@hypr/extension-live-summary/tailwind.config";
import ExtensionTranscript from "@hypr/extension-transcript/tailwind.config";
import ExtensionWorldClocks from "@hypr/extension-world-clocks/tailwind.config";
import ExtensionDinoGame from "@hypr/extension-dino-game/tailwind.config";

const config = {
  content: [
    ...Tiptap.content,
    ...ExtensionLiveSummary.content,
    ...ExtensionTranscript.content,
    ...ExtensionWorldClocks.content,
    ...ExtensionDinoGame.content,
    "src/**/*.{js,ts,jsx,tsx}",
    "index.html",
  ],
  theme: {
    extend: {
      fontFamily: {
        "racing-sans": ['"Racing Sans One"', "cursive"],
      },
    },
  },
  plugins: [typography],
} satisfies Config;

export default config;

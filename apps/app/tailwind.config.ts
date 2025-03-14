import typography from "@tailwindcss/typography";
import type { Config } from "tailwindcss";

import Tiptap from "@hypr/tiptap/editor/tailwind.config";

const config = {
  content: [
    ...Tiptap.content,
    "src/**/*.{js,ts,jsx,tsx}",
    "index.html",
  ],
  theme: {
    extend: {
      fontFamily: {
        "racing-sans": ["Racing Sans One", "cursive"],
      },
    },
  },
  plugins: [typography({ target: "modern" })],
} satisfies Config;

export default config;

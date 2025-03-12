import Tiptap from "@hypr/tiptap/editor/tailwind.config";
import typography from "@tailwindcss/typography";
import type { Config } from "tailwindcss";

const config = {
  content: [...Tiptap.content, "src/**/*.{js,ts,jsx,tsx}", "index.html"],
  theme: {
    extend: {
      fontFamily: {
        "racing-sans": ["Racing Sans One", "cursive"],
      },
    },
  },
  plugins: [
    typography,
  ],
} satisfies Config;

export default config;

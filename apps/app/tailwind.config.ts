import Tiptap from "@hypr/tiptap/editor/tailwind.config";
import UI from "@hypr/ui/tailwind.config";
import typography from "@tailwindcss/typography";
import type { Config } from "tailwindcss";

const config = {
  ...UI,
  content: [
    ...Tiptap.content,
    ...UI.content,
    "src/**/*.{js,ts,jsx,tsx}",
    "index.html",
  ],
  theme: {
    extend: {
      ...UI.theme?.extend,
      fontFamily: {
        "racing-sans": ["\"Racing Sans One\"", "cursive"],
      },
    },
  },
  plugins: [typography],
} satisfies Config;

export default config;

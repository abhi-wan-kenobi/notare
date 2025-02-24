import path from "path";
import type { Config } from "tailwindcss";
import UI from "@hypr/ui/tailwind.config";

const config = {
  ...UI,
  content: [...UI.content, path.resolve(__dirname, "./src/*.tsx")],
  theme: {
    extend: {
      ...UI.theme?.extend,
    },
  },
  plugins: [...UI.plugins],
} satisfies Config;

export default config;

import path from "path";
import type { Config } from "tailwindcss";

const config = {
  content: [
    path.resolve(__dirname, "./*.tsx"),
    path.resolve(__dirname, "../shared/*.{tsx,ts}"),
  ],
} satisfies Config;

export default config;

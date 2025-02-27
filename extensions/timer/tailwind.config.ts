import path from "path";
import type { Config } from "tailwindcss";

const config = {
  content: [
    path.resolve(__dirname, "./src/**/*.{js,jsx,ts,tsx}"),
  ],
} satisfies Config;

export default config;

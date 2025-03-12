import type { Config } from "tailwindcss";
import plugin from "tailwindcss/plugin";

const config = {
  content: ["src/**/*.{js,ts,jsx,tsx}", "index.html"],
  theme: {
    extend: {
      fontFamily: {
        "racing-sans": ["Racing Sans One", "cursive"],
      },
    },
  },
  plugins: [
    plugin(function({ addUtilities }) {
      const newUtilities = {
        ".scrollbar-hide": {
          "-ms-overflow-style": "none",
          "scrollbar-width": "none",
          "&::-webkit-scrollbar": {
            display: "none",
          },
          "WebkitOverflowScrolling": "touch",
        },
        ".touch-scroll": {
          "overflow": "scroll",
          "WebkitOverflowScrolling": "touch",
          "&::-webkit-scrollbar": {
            display: "none",
          },
        },
        ".overflow-mask": {
          "overflow": "hidden",
          "maskImage": "-webkit-radial-gradient(white, black)",
          "WebkitMaskImage": "-webkit-radial-gradient(white, black)",
        },
      };

      addUtilities(newUtilities);
    }),
  ],
} satisfies Config;

export default config;

import ExtensionCalculator from "@hypr/extension-calculator/tailwind.config";
import ExtensionDinoGame from "@hypr/extension-dino-game/tailwind.config";
import ExtensionSummary from "@hypr/extension-summary/tailwind.config";
import ExtensionTimer from "@hypr/extension-timer/tailwind.config";
import ExtensionTranscript from "@hypr/extension-transcript/tailwind.config";
import Tiptap from "@hypr/tiptap/editor/tailwind.config";
import typography from "@tailwindcss/typography";
import type { Config } from "tailwindcss";
import ExtensionClock from "../../extensions/clock/tailwind.config";

const config = {
  content: [
    ...Tiptap.content,
    ...ExtensionCalculator.content,
    ...ExtensionSummary.content,
    ...ExtensionTranscript.content,
    ...ExtensionClock.content,
    ...ExtensionDinoGame.content,
    ...ExtensionTimer.content,
    "src/**/*.{js,ts,jsx,tsx}",
    "index.html",
  ],
  theme: {
    extend: {
      fontFamily: {
        "racing-sans": ['Racing Sans One', "cursive"],
      },
    },
  },
  plugins: [typography],
} satisfies Config;

export default config;

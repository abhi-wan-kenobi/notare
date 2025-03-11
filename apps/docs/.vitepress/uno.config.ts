import { defineConfig } from "unocss";

import presetIcons from "@unocss/preset-icons";
import presetWind from "@unocss/preset-wind3";

export default defineConfig({
  presets: [
    presetWind({ important: true }),
    // https://icones.js.org
    presetIcons({
      collections: {
        heroicons: () => import("@iconify-json/heroicons/icons.json").then((i) => i.default),
      },
    }),
  ],
});

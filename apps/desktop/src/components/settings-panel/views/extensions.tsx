import { ExtensionDefinition } from "@hypr/plugin-db";

import calculatorConfig from "@hypr/extension-calculator/config.json";
import clockConfig from "@hypr/extension-clock/config.json";
import dinoGameConfig from "@hypr/extension-dino-game/config.json";
import summaryConfig from "@hypr/extension-summary/config.json";
import timerConfig from "@hypr/extension-timer/config.json";
import transcriptConfig from "@hypr/extension-transcript/config.json";

const EXTENSION_CONFIGS: ExtensionDefinition[] = [
  transcriptConfig,
  summaryConfig,
  timerConfig,
  calculatorConfig,
  clockConfig,
  dinoGameConfig,
];

export default function Extensions() {
  return (
    <div>
      {EXTENSION_CONFIGS.map((config) => (
        <div key={config.id}>
          <h3>{config.title}</h3>
          <p>{config.description}</p>
        </div>
      ))}
    </div>
  );
}

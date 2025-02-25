import { type Extension } from "@hypr/extension-utils";
import { WidgetTwoByOne } from "@hypr/ui/components/ui/widgets";
import DinoGame from "./dino-game";

const widget: Extension["twoByOne"] = () => (
  <WidgetTwoByOne>
    <div className="w-full h-full">
      <DinoGame />
    </div>
  </WidgetTwoByOne>
);

export default widget;

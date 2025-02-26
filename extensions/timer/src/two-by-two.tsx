import { type Extension } from "@hypr/extension-utils";
import { WidgetTwoByTwo } from "@hypr/ui/components/ui/widgets";
import { Timer } from "./timer";

const widget: Extension["twoByTwo"] = () => (
  <WidgetTwoByTwo>
    <Timer />
  </WidgetTwoByTwo>
);

export default widget;

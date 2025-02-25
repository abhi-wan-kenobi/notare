import { type Extension } from "@hypr/extension-utils";
import { WidgetTwoByOne } from "@hypr/ui/components/ui/widgets";

const widget: Extension["twoByTwo"] = () => (
  <WidgetTwoByOne>
    <DinoGame />
  </WidgetTwoByOne>
);

const DinoGame = () => {
  return <div></div>;
};

export default widget;

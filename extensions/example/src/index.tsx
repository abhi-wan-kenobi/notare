import { type Extension } from "@hypr/extension-utils";
import { WidgetTwoByTwo } from "@hypr/ui/components/ui/widgets";

import init from "./init";

const extension: Extension = {
  init,
  twoByTwo: () => <WidgetTwoByTwo>Example extension</WidgetTwoByTwo>,
};

export default extension;

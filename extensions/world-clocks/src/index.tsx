import { type Extension } from "@hypr/extension-utils";

const extension: Extension = {
  init: async () => {
    console.log("WorldClocks init");
  },
  panelSmall: () => <div>WorldClocks</div>,
  panelLarge: () => <div>WorldClocks</div>,
};

export default extension;

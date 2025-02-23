import { type Extension } from "@hypr/extension-utils";

const extension: Extension = {
  init: async () => {
    console.log("WorldClocks init");
  },
  panelTwoByTwo: () => <div>WorldClocks</div>,
};

export default extension;

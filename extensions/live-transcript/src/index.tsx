import { type Extension } from "@hypr/extension-utils";

import init from "./init";

const extension: Extension = {
  init,
  panelSmall: () => {
    return (
      <div>
        <h1>Live</h1>
      </div>
    );
  },
};

export default extension;

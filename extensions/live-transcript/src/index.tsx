import { type Extension } from "@hypr/extension-utils";

import init from "./init";

const extension: Extension = {
  init,
  panelTwoByTwo: () => {
    return (
      <div>
        <h1>qwdqwdqwdqw</h1>
      </div>
    );
  },
};

export default extension;

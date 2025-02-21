import type { Extension } from "../../types";

import init from "./init";

const extension: Extension = {
  init,
  modal: () => {
    return <div>Example extension</div>;
  },
};

export default extension;

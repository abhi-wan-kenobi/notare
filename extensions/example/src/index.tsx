import type { Extension } from "../../types";

const extension: Extension = {
  init: async () => {
    console.log("Example extension initialized");
  },
  modal: () => {
    return <div>Example extension</div>;
  },
};

export default extension;

import { type Extension } from "@hypr/extension-utils";

const init: Extension["init"] = async () => {
  console.log("Calculator extension initialized");
};

export default init;

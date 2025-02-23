import { type Extension } from "@hypr/extension-utils";

const init: Extension["init"] = async () => {
  console.log("Live transcript extension initialized");
};

export default init;

import { type Extension } from "@hypr/extension-utils";

const init: Extension["init"] = async () => {
  console.log("Transcript extension initialized");
};

export default init;

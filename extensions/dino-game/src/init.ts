import { type Extension } from "@hypr/extension-utils";

const init: Extension["init"] = async () => {
  console.log("Dino Game extension initialized");
};

export default init;

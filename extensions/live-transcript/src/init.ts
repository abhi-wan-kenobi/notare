import { Extension } from "../../types";

const init: Extension["init"] = async () => {
  console.log("Live transcript extension initialized");
};

export default init;

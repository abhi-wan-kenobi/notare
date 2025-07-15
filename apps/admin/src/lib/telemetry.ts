import posthog from "posthog-js";

import { envServerData } from "@/env";

export const init = () => {
  if (!envServerData.TELEMETRY) {
    return;
  }

  posthog.init("KEY", {
    api_host: "https://eu.posthog.com",
  }, "hypr_admin");
};

export const capture = (
  event_name: Parameters<typeof posthog.capture>[0],
  properties?: Parameters<typeof posthog.capture>[1],
  options?: Parameters<typeof posthog.capture>[2],
) => {
  if (!envServerData.TELEMETRY) {
    return;
  }

  posthog.capture(event_name, properties, options);
};

import { createServerFn } from "@tanstack/react-start";

import { ENV_KEYS_SERVER, envServerData } from "@/env";
import { z } from "zod";

export const getEnv = createServerFn({ method: "GET" })
  .validator(z.object({ key: z.enum(ENV_KEYS_SERVER) }))
  .handler(
    async ({ data }) => {
      return envServerData[data.key];
    },
  );

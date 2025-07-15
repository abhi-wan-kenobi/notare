import { apiKeyClient, ssoClient } from "better-auth/client/plugins";
import { organizationClient } from "better-auth/client/plugins";
import { createAuthClient } from "better-auth/react";

import { envServerSchema } from "@/env";

export const authClient = createAuthClient({
  baseURL: envServerSchema.BASE_URL,
  basePath: "/api/auth",
  plugins: [
    ssoClient(),
    organizationClient(),
    apiKeyClient(),
  ],
});

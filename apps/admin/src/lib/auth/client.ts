import { apiKeyClient, ssoClient } from "better-auth/client/plugins";
import { organizationClient } from "better-auth/client/plugins";
import { createAuthClient } from "better-auth/react";

import { envClientSchema } from "@/envClient";

export const authClient = createAuthClient({
  baseURL: envClientSchema.VITE_BASE_URL,
  basePath: "/api/auth",
  plugins: [
    ssoClient(),
    organizationClient(),
    apiKeyClient(),
  ],
});

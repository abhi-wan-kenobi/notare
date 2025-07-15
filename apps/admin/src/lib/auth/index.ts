import { betterAuth } from "better-auth";
import { drizzleAdapter } from "better-auth/adapters/drizzle";

import { apiKey, organization } from "better-auth/plugins";
import { sso } from "better-auth/plugins/sso";
import { reactStartCookies } from "better-auth/react-start";

import { envServerData } from "@/env";
import { db } from "@/lib/db";
import * as schema from "@/lib/db/schema";

export * from "./client";

export const auth = betterAuth({
  baseURL: envServerData.BASE_URL,
  basePath: "/api/auth",
  database: drizzleAdapter(db, { provider: "sqlite", schema }),
  plugins: [
    apiKey({
      customAPIKeyGetter(ctx) {
        const has = ctx.request?.headers?.has("authorization");
        if (!has) {
          return null;
        }

        const authorization = ctx.request?.headers?.get("authorization");
        if (!authorization?.startsWith("Bearer ")) {
          return null;
        }

        return authorization?.slice(7) ?? null;
      },
    }),
    sso(),
    organization({
      allowUserToCreateOrganization: async (user) => {
        return user.email === envServerData.ADMIN_EMAIL;
      },
    }),
    reactStartCookies(),
  ],
  emailAndPassword: {
    enabled: true,
  },
});

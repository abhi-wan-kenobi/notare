import { betterAuth } from "better-auth";
import { drizzleAdapter } from "better-auth/adapters/drizzle";

import { apiKey, organization } from "better-auth/plugins";
import { sso } from "better-auth/plugins/sso";
import { reactStartCookies } from "better-auth/react-start";

import { db } from "@/lib/db";

export const auth = betterAuth({
  database: drizzleAdapter(db, { provider: "sqlite" }),
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
    organization(),
    reactStartCookies(),
  ],
  emailAndPassword: {
    enabled: true,
  },
});

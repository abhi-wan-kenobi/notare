import { createServerFn } from "@tanstack/react-start";
import { getWebRequest } from "@tanstack/react-start/server";

import { auth } from "@/lib/auth";
import { db } from "@/lib/db";
import { organizationConfig } from "@/lib/db/schema/config";
import { eq } from "drizzle-orm";

export const getOrganizationConfig = createServerFn({ method: "GET" }).handler(
  async () => {
    const request = getWebRequest();

    if (!request?.headers) {
      return null;
    }

    const userSession = await auth.api.getSession({ headers: request.headers });
    if (!userSession || !userSession.session.activeOrganizationId) {
      return null;
    }

    const [config] = await db.select().from(organizationConfig).where(
      eq(organizationConfig.organizationId, userSession.session.activeOrganizationId),
    );

    return config;
  },
);

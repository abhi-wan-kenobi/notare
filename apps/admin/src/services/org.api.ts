import { createServerFn } from "@tanstack/react-start";

import { envServerData } from "@/env";
import { db } from "@/lib/db";
import { organization } from "@/lib/db/schema";

export const orgCreated = createServerFn({ method: "POST" }).handler(
  async () => {
    const organizations = await db.select().from(organization);
    return organizations.some((org) => org.slug === envServerData.ORG_SLUG);
  },
);

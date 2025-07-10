import { sqliteTable } from "drizzle-orm/sqlite-core";

import { shared } from "./shared";

export const device = sqliteTable("device", {
  ...shared,
});

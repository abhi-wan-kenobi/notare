import { sqliteTable, text } from "drizzle-orm/sqlite-core";
import { shared } from "./shared";

export const llmProvider = sqliteTable("llm_provider", {
  ...shared,
  name: text("name").notNull(),
  model: text("model").notNull(),
  baseUrl: text("base_url").notNull(),
  apiKey: text("api_key").notNull(),
});

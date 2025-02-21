import { z } from "zod";
import { zodToJsonSchema } from "zod-to-json-schema";

const liveSummaryResponseSchema = z.object({
  points: z.array(z.string()).min(1).max(3),
});

export const liveSummaryResponseJsonSchema = zodToJsonSchema(
  liveSummaryResponseSchema,
  "live_summary",
);

export type LiveSummaryResponse = z.infer<typeof liveSummaryResponseSchema>;

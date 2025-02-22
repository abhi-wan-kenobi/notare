import { z } from "zod";

export const liveSummaryResponseSchema = z.object({
  points: z.array(z.string()).min(1).max(3),
});

export type LiveSummaryResponse = z.infer<typeof liveSummaryResponseSchema>;

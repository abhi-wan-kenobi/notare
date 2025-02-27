import { z } from "zod";
import type { Config } from "@hypr/plugin-db";
import type { TimelineView } from "@hypr/plugin-listener";

export const liveSummaryResponseSchema = z.object({
  points: z.array(z.string()).min(1).max(3),
});

export type LiveSummaryResponse = z.infer<typeof liveSummaryResponseSchema>;

export type LiveSummarySystemInput = {
  config: Config;
};

export type LiveSummaryUserInput = {
  timeline: TimelineView;
};

import { z } from "zod";

import { type Tab } from "~/store/zustand/tabs";

export type DailySummaryTab = Extract<Tab, { type: "daily_summary" }>;

export const dailySummarySchema = z.object({
  summaryMd: z.string().min(1),
  topics: z
    .array(
      z.object({
        title: z.string().min(1),
        summary: z.string().min(1),
      }),
    )
    .max(8),
  timeline: z
    .array(
      z.object({
        time: z.string().min(1),
        summary: z.string().min(1),
      }),
    )
    .max(12),
});

export type ActivityCaptureEntry =
  | {
      kind: "analysis";
      capturedAtMs: number;
      fingerprint: string;
      payload: {
        appName: string;
        windowTitle: string | null;
        summary: string;
        reason: string;
      };
    }
  | {
      kind: "error";
      capturedAtMs: number;
      fingerprint: string;
      payload: {
        appName: string;
        windowTitle: string | null;
        message: string;
      };
    };

export type FeedTab = "timeline" | "raw";

export interface GenerateHandle {
  mutate: () => void;
  isPending: boolean;
  canGenerate: boolean;
}

import { commands as calendarCommands } from "@hypr/plugin-calendar";
import { commands as openerCommands } from "@hypr/plugin-opener2";

import { buildMailtoUrl } from "./mailto";

export interface DeliveryResult {
  method: "gmail" | "mailto";
  delivered?: "gmail" | "mailto";
  draftId?: string;
  mailtoUrl?: string;
  reason?: string;
  fallbackReason?: string;
}

/**
 * Deliver follow-up email:
 * 1. If the Gmail client is connected with an active account, create a draft in Gmail.
 * 2. Otherwise, copy body to clipboard and open default email client via mailto: URL.
 * Never auto-sends.
 */
export async function deliverFollowupEmail({
  to,
  subject,
  body,
  accountEmail,
}: {
  to: string[];
  subject: string;
  body: string;
  accountEmail?: string | null;
}): Promise<DeliveryResult> {
  const recipients = to.filter(Boolean).map((e) => e.trim());
  if (recipients.length === 0) {
    throw new Error("No recipients specified for follow-up email");
  }

  let isGmailConnected = false;

  try {
    const statusResult = await calendarCommands.gmailStatus();
    if (
      statusResult.status === "ok" &&
      statusResult.data.has_client &&
      statusResult.data.connected &&
      Array.isArray(statusResult.data.accounts) &&
      statusResult.data.accounts.length > 0
    ) {
      isGmailConnected = true;
    }
  } catch {
    isGmailConnected = false;
  }

  let gmailFailureReason: string | undefined;

  if (isGmailConnected) {
    try {
      const draftResult = await calendarCommands.gmailCreateDraft(
        accountEmail ?? null,
        recipients,
        subject,
        body,
      );
      if (draftResult.status === "ok") {
        return {
          method: "gmail",
          delivered: "gmail",
          draftId: draftResult.data,
        };
      }
      gmailFailureReason = draftResult.error ?? "Failed to create Gmail draft";
    } catch (e) {
      gmailFailureReason = e instanceof Error ? e.message : String(e);
      console.warn("failed to create Gmail draft; falling back to mailto", e);
    }
  }

  // Fallback (b): copy to clipboard and open mailto
  if (typeof navigator !== "undefined" && navigator.clipboard?.writeText) {
    try {
      await navigator.clipboard.writeText(body);
    } catch (e) {
      console.warn("clipboard writeText failed", e);
    }
  }

  const mailtoUrl = buildMailtoUrl(recipients, subject, body);
  try {
    await openerCommands.openUrl(mailtoUrl, null);
  } catch (e) {
    console.warn("openUrl failed", e);
  }

  return {
    method: "mailto",
    delivered: "mailto",
    mailtoUrl,
    ...(gmailFailureReason
      ? { reason: gmailFailureReason, fallbackReason: gmailFailureReason }
      : {}),
  };
}

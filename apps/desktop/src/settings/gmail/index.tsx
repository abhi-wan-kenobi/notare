import { Trans, useLingui } from "@lingui/react/macro";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  AlertCircleIcon,
  CheckCircle2Icon,
  Loader2Icon,
  MailIcon,
} from "lucide-react";
import { useState } from "react";

import {
  commands as calendarCommands,
  type GmailAccountStatus,
} from "@hypr/plugin-calendar";
import { Button } from "@hypr/ui/components/ui/button";

import { SettingsPageTitle } from "~/settings/page-title";

const GMAIL_STATUS_QUERY_KEY = ["settings", "gmail", "status"] as const;

export function SettingsGmail() {
  const { t } = useLingui();
  const queryClient = useQueryClient();
  const [actionError, setActionError] = useState<string | null>(null);

  const {
    data: status,
    isLoading,
    error,
  } = useQuery({
    queryKey: GMAIL_STATUS_QUERY_KEY,
    queryFn: async (): Promise<GmailAccountStatus> => {
      const result = await calendarCommands.gmailStatus();
      if (result.status === "error") throw new Error(result.error);
      return result.data;
    },
  });

  const invalidate = () =>
    queryClient.invalidateQueries({ queryKey: GMAIL_STATUS_QUERY_KEY });

  const connectMutation = useMutation({
    mutationFn: async () => {
      const result = await calendarCommands.gmailConnect();
      if (result.status === "error") throw new Error(result.error);
      return result.data;
    },
    onSuccess: () => {
      setActionError(null);
      void invalidate();
    },
    onError: (err: Error) => setActionError(err.message),
  });

  const disconnectMutation = useMutation({
    mutationFn: async (email?: string | null) => {
      const result = await calendarCommands.gmailDisconnect(email ?? null);
      if (result.status === "error") throw new Error(result.error);
      return result.data;
    },
    onSuccess: () => {
      setActionError(null);
      void invalidate();
    },
    onError: (err: Error) => setActionError(err.message),
  });

  if (isLoading || !status) {
    return (
      <div className="flex min-h-48 items-center justify-center">
        <Loader2Icon
          aria-label={t`Loading Gmail settings`}
          className="text-muted-foreground size-5 animate-spin"
        />
      </div>
    );
  }

  if (error) {
    return (
      <div className="flex flex-col gap-2 p-4 text-red-600">
        <p>{(error as Error).message}</p>
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-6">
      <div className="flex flex-col gap-2">
        <SettingsPageTitle title={t`Gmail (beta)`} />
        <p className="text-muted-foreground max-w-xl text-xs leading-relaxed">
          <Trans>
            Connect your Google account to create follow-up email drafts
            directly in Gmail after meetings. This integration is separate from
            Google Calendar and uses its own OAuth client requesting only
            draft-creation permissions (
            <code className="font-mono text-[11px]">gmail.compose</code>).
            Notare never reads your inbox or sends emails automatically.
          </Trans>
        </p>
      </div>

      {!status.has_client ? (
        <div className="border-border bg-card/60 flex items-start gap-3 rounded-lg border p-4">
          <AlertCircleIcon className="mt-0.5 size-4 shrink-0 text-amber-500" />
          <div className="flex flex-col gap-1 text-xs">
            <span className="text-foreground font-medium">
              <Trans>Gmail OAuth client not configured in this build</Trans>
            </span>
            <p className="text-muted-foreground leading-relaxed">
              <Trans>
                Official builds include the second Gmail client credentials. In
                development or source builds without{" "}
                <code className="font-mono text-[11px]">
                  NOTARE_GMAIL_CLIENT_ID
                </code>
                , follow-up email drafts seamlessly fall back to clipboard copy
                and your system email app (
                <code className="font-mono text-[11px]">mailto:</code>
                ).
              </Trans>
            </p>
          </div>
        </div>
      ) : (
        <div className="flex max-w-xl flex-col gap-4">
          <div className="border-border bg-card/40 flex flex-col gap-3 rounded-lg border p-4">
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-2">
                <MailIcon className="text-muted-foreground size-4" />
                <span className="text-foreground text-sm font-medium">
                  <Trans>Connected Accounts</Trans>
                </span>
              </div>
              <Button
                size="sm"
                onClick={() => connectMutation.mutate()}
                disabled={connectMutation.isPending}
              >
                {connectMutation.isPending ? (
                  <>
                    <Loader2Icon className="mr-1 size-3 animate-spin" />
                    <Trans>Connecting…</Trans>
                  </>
                ) : (
                  <Trans>Connect Account</Trans>
                )}
              </Button>
            </div>

            {(status.accounts ?? []).length === 0 ? (
              <p className="text-muted-foreground text-xs italic">
                <Trans>No Gmail accounts connected yet.</Trans>
              </p>
            ) : (
              <div className="divide-border/60 flex flex-col divide-y">
                {(status.accounts ?? []).map((email) => (
                  <div
                    key={email}
                    className="flex items-center justify-between py-2 text-xs"
                  >
                    <div className="flex items-center gap-2">
                      <CheckCircle2Icon className="size-3.5 text-emerald-500" />
                      <span className="text-foreground font-mono">{email}</span>
                    </div>
                    <Button
                      variant="ghost"
                      size="sm"
                      onClick={() => disconnectMutation.mutate(email)}
                      disabled={disconnectMutation.isPending}
                      className="h-7 px-2 text-xs text-red-500 hover:bg-red-50 hover:text-red-700 dark:hover:bg-red-950/30"
                    >
                      <Trans>Disconnect</Trans>
                    </Button>
                  </div>
                ))}
              </div>
            )}
          </div>

          {actionError && (
            <div className="rounded-md border border-red-200 bg-red-50 p-2.5 text-xs text-red-700 dark:border-red-900/50 dark:bg-red-950/20 dark:text-red-400">
              {actionError}
            </div>
          )}
        </div>
      )}
    </div>
  );
}

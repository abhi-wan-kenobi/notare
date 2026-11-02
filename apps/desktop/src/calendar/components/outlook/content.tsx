import { Trans } from "@lingui/react/macro";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useCallback, useState } from "react";

import { commands as calendarCommands } from "@hypr/plugin-calendar";
import type { OutlookAccountStatus } from "@hypr/plugin-calendar";
import { commands as openerCommands } from "@hypr/plugin-opener2";

import { useSync } from "../context";
import {
  OAuthCalendarSelection,
  useOAuthCalendarSelection,
} from "../oauth/calendar-selection";
import type { CalendarProvider } from "../shared";

const STATUS_QUERY_KEY = ["outlook-calendar-account"];

function useOutlookAccountStatus() {
  return useQuery({
    queryKey: STATUS_QUERY_KEY,
    queryFn: async (): Promise<OutlookAccountStatus> => {
      const result = await calendarCommands.outlookAccountStatus();
      if (result.status === "error") throw new Error(result.error);
      return result.data;
    },
  });
}

/**
 * Direct ("bring your own OAuth client") Outlook Calendar connect UI.
 */
export function OutlookDirectContent({ config }: { config: CalendarProvider }) {
  const queryClient = useQueryClient();
  const { data: status, isPending, isError } = useOutlookAccountStatus();
  const { scheduleSync } = useSync();
  const [actionError, setActionError] = useState<string | null>(null);
  const [showAdvanced, setShowAdvanced] = useState(false);

  const invalidateStatus = useCallback(
    () => queryClient.invalidateQueries({ queryKey: STATUS_QUERY_KEY }),
    [queryClient],
  );

  const setClientIdMutation = useMutation({
    mutationFn: async (clientId: string) => {
      const result = await calendarCommands.outlookSetClientId(clientId);
      if (result.status === "error") throw new Error(result.error);
      return result.data;
    },
    onSuccess: () => {
      setActionError(null);
      void invalidateStatus();
    },
    onError: (error: Error) => setActionError(error.message),
  });

  const connectMutation = useMutation({
    mutationFn: async () => {
      const result = await calendarCommands.outlookConnect();
      if (result.status === "error") throw new Error(result.error);
      return result.data;
    },
    onSuccess: () => {
      setActionError(null);
      void invalidateStatus();
      scheduleSync();
    },
    onError: (error: Error) => setActionError(error.message),
  });

  const disconnectMutation = useMutation({
    mutationFn: async (email?: string | null) => {
      const result = await calendarCommands.outlookDisconnect(email ?? null);
      if (result.status === "error") throw new Error(result.error);
      return result.data;
    },
    onSuccess: () => {
      setActionError(null);
      void invalidateStatus();
      scheduleSync();
    },
    onError: (error: Error) => setActionError(error.message),
  });

  const resetMutation = useMutation({
    mutationFn: async () => {
      const result = await calendarCommands.outlookReset();
      if (result.status === "error") throw new Error(result.error);
      return result.data;
    },
    onSuccess: () => {
      setActionError(null);
      void invalidateStatus();
    },
    onError: (error: Error) => setActionError(error.message),
  });

  if (isPending) {
    return (
      <div className="pt-1 pb-2">
        <span className="text-muted-foreground text-xs">Loading…</span>
      </div>
    );
  }

  if (isError || !status) {
    return (
      <div className="pt-1 pb-2">
        <span className="text-xs text-red-600">
          Failed to load Outlook Calendar status
        </span>
      </div>
    );
  }

  if (!status.has_client) {
    if (status.has_bundled_client) {
      return (
        <div className="flex flex-col gap-2 pt-1 pb-2">
          <button
            onClick={() => connectMutation.mutate()}
            disabled={connectMutation.isPending}
            className="border-border hover:bg-accent inline-flex w-fit items-center justify-center gap-2 rounded-md border px-3 py-1.5 text-xs font-medium transition-colors disabled:cursor-default disabled:opacity-50"
          >
            {connectMutation.isPending
              ? "Waiting for Microsoft… finish sign-in in your browser"
              : "Sign in with Microsoft"}
          </button>
          {!showAdvanced ? (
            <button
              onClick={() => setShowAdvanced(true)}
              className="text-muted-foreground hover:text-foreground w-fit cursor-pointer text-[11px] underline transition-colors"
            >
              Advanced: use your own Application (client) ID
            </button>
          ) : (
            <BYOClientIdContent
              onSaveClientId={(clientId) =>
                setClientIdMutation.mutate(clientId)
              }
              isSaving={setClientIdMutation.isPending}
              error={actionError}
              docsPath={config.docsPath}
            />
          )}
          {actionError && <p className="text-xs text-red-600">{actionError}</p>}
        </div>
      );
    }
    return (
      <BYOClientIdContent
        onSaveClientId={(clientId) => setClientIdMutation.mutate(clientId)}
        isSaving={setClientIdMutation.isPending}
        error={actionError}
        docsPath={config.docsPath}
      />
    );
  }

  if (!status.connected) {
    return (
      <div className="flex flex-col gap-2 pt-1 pb-2">
        <p className="text-muted-foreground truncate text-xs">
          Client ID: {status.client_id}
        </p>
        <div className="flex items-center gap-2">
          <button
            onClick={() => connectMutation.mutate()}
            disabled={connectMutation.isPending}
            className="text-muted-foreground hover:text-foreground cursor-pointer text-xs underline transition-colors disabled:cursor-default disabled:no-underline"
          >
            {connectMutation.isPending
              ? "Waiting for Microsoft… finish sign-in in your browser"
              : `Connect ${config.displayName} Calendar`}
          </button>
          {!connectMutation.isPending && (
            <>
              <span className="text-muted-foreground text-xs">or</span>
              <button
                onClick={() => resetMutation.mutate()}
                className="cursor-pointer text-xs text-red-500 underline transition-colors hover:text-red-700"
              >
                Remove client
              </button>
            </>
          )}
        </div>
        {actionError && <p className="text-xs text-red-600">{actionError}</p>}
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-2 pb-2">
      <ConnectedOutlookContent
        config={config}
        accounts={status.accounts ?? []}
        onConnect={() => connectMutation.mutate()}
        onDisconnect={(email) => disconnectMutation.mutate(email)}
        onReset={() => resetMutation.mutate()}
        isConnecting={connectMutation.isPending}
      />
      {actionError && <p className="text-xs text-red-600">{actionError}</p>}
    </div>
  );
}

const ENTRA_APP_REGISTRATIONS_URL =
  "https://entra.microsoft.com/#view/Microsoft_AAD_RegisteredApps/CreateApplicationBlade";

function SetupGuide({ docsPath }: { docsPath: string | undefined }) {
  const [open, setOpen] = useState(false);

  return (
    <div className="flex flex-col gap-1">
      <button
        onClick={() => setOpen((v) => !v)}
        aria-expanded={open}
        className="text-muted-foreground hover:text-foreground cursor-pointer self-start text-xs underline transition-colors"
      >
        {open ? (
          <Trans>Hide setup steps</Trans>
        ) : (
          <Trans>How do I register an app?</Trans>
        )}
      </button>
      {open && (
        <div className="border-border bg-muted/40 flex flex-col gap-2 rounded-md border p-2">
          <ol className="text-muted-foreground list-decimal space-y-1 pl-4 text-xs">
            <li>
              <Trans>
                Go to the Microsoft Entra admin center or Azure Portal → App
                registrations → New registration.
              </Trans>
            </li>
            <li>
              <Trans>
                Name:{" "}
                <span className="text-foreground font-medium">Notare</span> (or
                any name).
              </Trans>
            </li>
            <li>
              <Trans>
                Supported account types: choose{" "}
                <span className="text-foreground font-medium">
                  Accounts in any organizational directory and personal
                  Microsoft accounts
                </span>
                .
              </Trans>
            </li>
            <li>
              <Trans>
                Redirect URI: select platform{" "}
                <span className="text-foreground font-medium">
                  Mobile and desktop applications
                </span>{" "}
                and enter{" "}
                <code className="font-mono text-xs">http://localhost</code>.
              </Trans>
            </li>
            <li>
              <Trans>
                Click Register, then copy the{" "}
                <span className="text-foreground font-medium">
                  Application (client) ID
                </span>{" "}
                and paste it below.
              </Trans>
            </li>
          </ol>
          <div className="flex items-center gap-2 pt-1">
            <button
              onClick={() =>
                void openerCommands.openUrl(ENTRA_APP_REGISTRATIONS_URL, null)
              }
              className="text-muted-foreground hover:text-foreground cursor-pointer text-xs underline transition-colors"
            >
              <Trans>Open Entra admin center</Trans>
            </button>
            {docsPath && (
              <>
                <span className="text-muted-foreground text-xs">·</span>
                <button
                  onClick={() => void openerCommands.openUrl(docsPath, null)}
                  className="text-muted-foreground hover:text-foreground cursor-pointer text-xs underline transition-colors"
                >
                  <Trans>Full setup guide</Trans>
                </button>
              </>
            )}
          </div>
        </div>
      )}
    </div>
  );
}

function BYOClientIdContent({
  onSaveClientId,
  isSaving,
  error,
  docsPath,
}: {
  onSaveClientId: (clientId: string) => void;
  isSaving: boolean;
  error: string | null;
  docsPath: string | undefined;
}) {
  const [clientId, setClientId] = useState("");

  return (
    <div className="flex flex-col gap-2 pt-1 pb-2">
      <p className="text-muted-foreground text-xs">
        Connect with your own Microsoft Entra OAuth client — no cloud proxy,
        direct local connection.
      </p>
      <SetupGuide docsPath={docsPath} />
      <div className="flex flex-col gap-1.5">
        <label
          htmlFor="outlook-client-id"
          className="text-muted-foreground text-xs"
        >
          Application (client) ID:
        </label>
        <div className="flex items-center gap-2">
          <input
            id="outlook-client-id"
            type="text"
            value={clientId}
            onChange={(e) => setClientId(e.target.value.trim())}
            placeholder="00000000-0000-0000-0000-000000000000"
            className="border-border bg-background w-full rounded-md border px-2.5 py-1 font-mono text-xs"
            spellCheck={false}
          />
          <button
            onClick={() => onSaveClientId(clientId)}
            disabled={isSaving || clientId.length === 0}
            className="border-border hover:bg-accent shrink-0 rounded-md border px-3 py-1 text-xs font-medium transition-colors disabled:opacity-50"
          >
            {isSaving ? "Saving…" : "Save"}
          </button>
        </div>
      </div>
      {error && <p className="text-xs text-red-600">{error}</p>}
    </div>
  );
}

function ConnectedOutlookContent({
  config,
  accounts,
  onConnect,
  onDisconnect,
  onReset,
  isConnecting,
}: {
  config: CalendarProvider;
  accounts: OutlookAccountStatus["accounts"];
  onConnect: () => void;
  onDisconnect: (email?: string) => void;
  onReset: () => void;
  isConnecting: boolean;
}) {
  const { groups, handleRefresh, handleToggle, isLoading } =
    useOAuthCalendarSelection(config);

  return (
    <div className="flex flex-col gap-3">
      <div className="border-border bg-card/50 flex flex-col gap-1.5 rounded-md border p-2">
        <span className="text-muted-foreground text-[11px] font-medium tracking-wider uppercase">
          Connected Accounts
        </span>
        <div className="flex flex-col gap-1">
          {(accounts ?? []).map((acc) => (
            <div
              key={acc.email}
              className="hover:bg-accent/40 flex items-center justify-between rounded px-1.5 py-1 text-xs"
            >
              <span
                className="max-w-[200px] truncate font-mono text-xs"
                title={acc.email}
              >
                {acc.email}
              </span>
              <button
                onClick={() => onDisconnect(acc.email)}
                className="cursor-pointer text-xs text-red-500 underline hover:text-red-700"
              >
                Remove
              </button>
            </div>
          ))}
        </div>
        <button
          onClick={onConnect}
          disabled={isConnecting}
          className="text-muted-foreground hover:text-foreground cursor-pointer self-start pt-1 text-xs underline disabled:opacity-50"
        >
          {isConnecting ? "Waiting for browser…" : "+ Add Microsoft account"}
        </button>
      </div>

      <OAuthCalendarSelection
        groups={groups}
        onToggle={handleToggle}
        onRefresh={handleRefresh}
        isLoading={isLoading}
      />

      <div className="flex items-center gap-3 pt-1">
        <button
          onClick={() => onDisconnect()}
          className="cursor-pointer text-xs text-red-500 underline hover:text-red-700"
        >
          Disconnect all
        </button>
        <span className="text-muted-foreground text-xs">•</span>
        <button
          onClick={onReset}
          className="text-muted-foreground cursor-pointer text-xs underline hover:text-red-500"
        >
          Disconnect & remove client
        </button>
      </div>
    </div>
  );
}

import { Trans } from "@lingui/react/macro";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { open as selectFile } from "@tauri-apps/plugin-dialog";
import { RefreshCwIcon } from "lucide-react";
import { useCallback, useState } from "react";

import { commands as calendarCommands } from "@hypr/plugin-calendar";
import type { GoogleAccountStatus } from "@hypr/plugin-calendar";
import { commands as openerCommands } from "@hypr/plugin-opener2";
import { Button } from "@hypr/ui/components/ui/button";
import { Switch } from "@hypr/ui/components/ui/switch";

import {
  setCalendarProviderHidden,
  useHiddenCalendarProviders,
} from "~/calendar/hidden-providers";
import { SettingsExpander } from "~/settings/components";
import { useSync } from "../context";
import {
  OAuthCalendarSelection,
  useOAuthCalendarSelection,
} from "../oauth/calendar-selection";
import type { CalendarProvider } from "../shared";
const STATUS_QUERY_KEY = ["google-calendar-account"];

function useGoogleAccountStatus() {
  return useQuery({
    queryKey: STATUS_QUERY_KEY,
    queryFn: async (): Promise<GoogleAccountStatus> => {
      const result = await calendarCommands.googleAccountStatus();
      if (result.status === "error") throw new Error(result.error);
      return result.data;
    },
  });
}

/**
 * Direct ("bring your own OAuth client") Google Calendar connect UI.
 *
 * Flow: import the client_secret_*.json downloaded from the Google Cloud
 * console (file picker first, paste as fallback) -> "Connect Google Calendar"
 * opens the browser consent screen -> calendars appear as toggles.
 */
export function GoogleDirectContent({ config }: { config: CalendarProvider }) {
  const queryClient = useQueryClient();
  const { data: status, isPending, isError } = useGoogleAccountStatus();
  const { scheduleSync } = useSync();
  const [actionError, setActionError] = useState<string | null>(null);
  // Reveal the BYO-client import UI when a bundled client is available (it's the
  // "Advanced" path — most users just click "Sign in with Google").
  const [showAdvanced, setShowAdvanced] = useState(false);

  const invalidateStatus = useCallback(
    () => queryClient.invalidateQueries({ queryKey: STATUS_QUERY_KEY }),
    [queryClient],
  );

  const importFileMutation = useMutation({
    mutationFn: async () => {
      const path = await selectFile({
        multiple: false,
        directory: false,
        filters: [{ name: "Google client JSON", extensions: ["json"] }],
      });
      if (!path) return null;
      const result = await calendarCommands.googleImportClientFile(
        path as string,
      );
      if (result.status === "error") throw new Error(result.error);
      return result.data;
    },
    onSuccess: (data) => {
      if (data) {
        setActionError(null);
        void invalidateStatus();
      }
    },
    onError: (error: Error) => setActionError(error.message),
  });

  const importJsonMutation = useMutation({
    mutationFn: async (json: string) => {
      const result = await calendarCommands.googleImportClientJson(json);
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
      const result = await calendarCommands.googleConnect();
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
      const result = await calendarCommands.googleDisconnect(email ?? null);
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
      const result = await calendarCommands.googleReset();
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
          Failed to load Google Calendar status
        </span>
      </div>
    );
  }

  if (!status.has_client) {
    // A bundled first-party client is compiled in → let the user sign in
    // directly, no Google Cloud project needed. BYO import stays under Advanced.
    if (status.has_bundled_client) {
      return (
        <div className="flex flex-col gap-2 pt-1 pb-2">
          <button
            onClick={() => connectMutation.mutate()}
            disabled={connectMutation.isPending}
            className="border-border hover:bg-accent inline-flex w-fit items-center justify-center gap-2 rounded-md border px-3 py-1.5 text-xs font-medium transition-colors disabled:cursor-default disabled:opacity-50"
          >
            {connectMutation.isPending
              ? "Waiting for Google… finish sign-in in your browser"
              : `Sign in with Google`}
          </button>
          {!showAdvanced ? (
            <button
              onClick={() => setShowAdvanced(true)}
              className="text-muted-foreground hover:text-foreground w-fit cursor-pointer text-[11px] underline transition-colors"
            >
              Advanced: use your own OAuth client
            </button>
          ) : (
            <ImportClientContent
              onSelectFile={() => importFileMutation.mutate()}
              onPasteJson={(json) => importJsonMutation.mutate(json)}
              isImporting={
                importFileMutation.isPending || importJsonMutation.isPending
              }
              error={actionError}
              docsPath={config.docsPath}
            />
          )}
          {actionError && <p className="text-xs text-red-600">{actionError}</p>}
        </div>
      );
    }
    return (
      <ImportClientContent
        onSelectFile={() => importFileMutation.mutate()}
        onPasteJson={(json) => importJsonMutation.mutate(json)}
        isImporting={
          importFileMutation.isPending || importJsonMutation.isPending
        }
        error={actionError}
        docsPath={config.docsPath}
      />
    );
  }

  if (!status.connected) {
    return (
      <div className="flex flex-col gap-2 pt-1 pb-2">
        {status.client_kind === "web" && (
          <p className="text-xs text-amber-700">
            This looks like a “Web application” OAuth client. Use a “Desktop
            app” client if connecting fails.
          </p>
        )}
        <p className="text-muted-foreground truncate text-xs">
          Client: {status.client_id}
        </p>
        <div className="flex items-center gap-2">
          <button
            onClick={() => connectMutation.mutate()}
            disabled={connectMutation.isPending}
            className="text-muted-foreground hover:text-foreground cursor-pointer text-xs underline transition-colors disabled:cursor-default disabled:no-underline"
          >
            {connectMutation.isPending
              ? "Waiting for Google… finish sign-in in your browser"
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
      <ConnectedGoogleContent
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

const GOOGLE_CREDENTIALS_URL =
  "https://console.cloud.google.com/apis/credentials";

/**
 * Compact, collapsible in-app version of docs/GOOGLE-CALENDAR.md for the
 * pre-connect state, so nobody has to leave the app to figure out where the
 * client JSON comes from.
 */
function SetupGuide({ docsPath }: { docsPath: string | undefined }) {
  return (
    <SettingsExpander
      title={<Trans>How do I get this file?</Trans>}
      description={<Trans>6 setup steps in Google Cloud console</Trans>}
    >
      <div className="border-border bg-muted/40 flex flex-col gap-2 rounded-md border p-3">
        <ol className="text-muted-foreground list-decimal space-y-1 pl-4 text-xs">
          <li>
            <Trans>
              Create a project in the Google Cloud console (any name, it's
              free).
            </Trans>
          </li>
          <li>
            <Trans>Enable the Google Calendar API for that project.</Trans>
          </li>
          <li>
            <Trans>
              Set up the OAuth consent screen: audience{" "}
              <span className="text-foreground font-medium">External</span>,
              then add your own Gmail address as a test user.
            </Trans>
          </li>
          <li>
            <Trans>
              Go to Credentials → Create credentials → OAuth client ID.
            </Trans>
          </li>
          <li>
            <Trans>
              Application type:{" "}
              <span className="text-foreground font-medium">Desktop app</span>{" "}
              — not “Web application”.
            </Trans>
          </li>
          <li>
            <Trans>Download the JSON, then select it here.</Trans>
          </li>
        </ol>
        <p className="text-xs text-amber-700">
          <Trans>
            If you see redirect URI / JavaScript origin fields, you picked Web
            — choose Desktop app instead.
          </Trans>
        </p>
        <div className="flex items-center gap-2 pt-1">
          <button
            type="button"
            onClick={() =>
              void openerCommands.openUrl(GOOGLE_CREDENTIALS_URL, null)
            }
            className="text-muted-foreground hover:text-foreground cursor-pointer text-xs underline transition-colors"
          >
            <Trans>Open Google Cloud console</Trans>
          </button>
          <span className="text-muted-foreground text-xs">·</span>
          {docsPath && (
            <button
              type="button"
              onClick={() => void openerCommands.openUrl(docsPath, null)}
              className="text-muted-foreground hover:text-foreground cursor-pointer text-xs underline transition-colors"
            >
              <Trans>Full setup guide</Trans>
            </button>
          )}
        </div>
      </div>
    </SettingsExpander>
  );
}

function ImportClientContent({
  onSelectFile,
  onPasteJson,
  isImporting,
  error,
  docsPath,
}: {
  onSelectFile: () => void;
  onPasteJson: (json: string) => void;
  isImporting: boolean;
  error: string | null;
  docsPath: string | undefined;
}) {
  const [showPaste, setShowPaste] = useState(false);
  const [pasted, setPasted] = useState("");

  return (
    <div className="flex flex-col gap-2 pt-1 pb-2">
      <p className="text-muted-foreground text-xs">
        Connect with your own Google OAuth client — no account with us, no
        cloud.
      </p>
      <SetupGuide docsPath={docsPath} />
      <div className="flex items-center gap-2">
        <button
          onClick={onSelectFile}
          disabled={isImporting}
          className="text-muted-foreground hover:text-foreground cursor-pointer text-xs underline transition-colors disabled:cursor-default"
        >
          Select your Google client JSON…
        </button>
        <span className="text-muted-foreground text-xs">or</span>
        <button
          onClick={() => setShowPaste((v) => !v)}
          className="text-muted-foreground hover:text-foreground cursor-pointer text-xs underline transition-colors"
        >
          paste it
        </button>
      </div>
      {showPaste && (
        <div className="flex flex-col gap-1">
          <textarea
            value={pasted}
            onChange={(e) => setPasted(e.target.value)}
            onDrop={(e) => {
              const file = e.dataTransfer?.files?.[0];
              if (file) {
                e.preventDefault();
                void file.text().then(setPasted);
              }
            }}
            placeholder='{"installed":{"client_id":"…"}}'
            rows={4}
            spellCheck={false}
            className="border-border bg-background w-full rounded-md border p-2 font-mono text-xs"
          />
          <button
            onClick={() => onPasteJson(pasted)}
            disabled={isImporting || pasted.trim().length === 0}
            className="text-muted-foreground hover:text-foreground cursor-pointer self-start text-xs underline transition-colors disabled:cursor-default disabled:opacity-50"
          >
            Import pasted JSON
          </button>
        </div>
      )}
      {error && <p className="text-xs text-red-600">{error}</p>}
    </div>
  );
}

function ConnectedGoogleContent({
  config,
  accounts,
  onConnect,
  onDisconnect,
  onReset,
  isConnecting,
}: {
  config: CalendarProvider;
  accounts: GoogleAccountStatus["accounts"];
  onConnect: () => void;
  onDisconnect: (email?: string) => void;
  onReset: () => void;
  isConnecting: boolean;
}) {
  const { groups, handleRefresh, handleToggle, isLoading } =
    useOAuthCalendarSelection(config);
  const { scheduleSync } = useSync();
  const hiddenProviderIds = useHiddenCalendarProviders();
  const isHidden = hiddenProviderIds.includes(config.id);
  const [isSyncing, setIsSyncing] = useState(false);

  const handleManualSync = useCallback(() => {
    setIsSyncing(true);
    scheduleSync();
    handleRefresh();
    setTimeout(() => setIsSyncing(false), 800);
  }, [handleRefresh, scheduleSync]);

  return (
    <div className="flex flex-col gap-3">
      {/* Status Bar: Connected State + Sync Now Ghost Pill */}
      <div className="flex items-center justify-between gap-2 border-border/60 border-b pb-2">
        <div className="flex items-center gap-2">
          <span className="size-2 rounded-full bg-emerald-500 shrink-0" />
          <span className="text-xs font-medium text-emerald-600">
            <Trans>Connected</Trans>
          </span>
        </div>
        <Button
          variant="ghost"
          size="sm"
          onClick={handleManualSync}
          disabled={isSyncing || isLoading}
          className="rounded-full border border-border/60 hover:bg-muted text-xs h-7 px-3 text-muted-foreground hover:text-foreground"
        >
          <RefreshCwIcon
            className={`mr-1.5 size-3 ${isSyncing || isLoading ? "animate-spin" : ""}`}
          />
          <Trans>Sync now</Trans>
        </Button>
      </div>

      {/* Connected Accounts */}
      <div className="border-border bg-card/50 flex flex-col gap-1.5 rounded-md border p-2.5">
        <span className="text-muted-foreground text-[10px] font-semibold tracking-wider uppercase">
          <Trans>Connected Accounts</Trans>
        </span>
        <div className="flex flex-col gap-1">
          {(accounts ?? []).map((acc) => (
            <div
              key={acc.email}
              className="hover:bg-accent/40 flex items-center justify-between rounded px-2 py-1 text-xs"
            >
              <span
                className="max-w-[220px] truncate font-mono text-xs"
                title={acc.email}
              >
                {acc.email}
              </span>
              <Button
                variant="ghost"
                size="sm"
                onClick={() => onDisconnect(acc.email)}
                className="h-6 px-2 text-xs text-destructive hover:bg-destructive/10 hover:text-destructive"
              >
                <Trans>Disconnect</Trans>
              </Button>
            </div>
          ))}
        </div>
        <button
          type="button"
          onClick={onConnect}
          disabled={isConnecting}
          className="text-muted-foreground hover:text-foreground cursor-pointer self-start pt-1 text-xs underline disabled:opacity-50"
        >
          {isConnecting ? "Waiting for browser…" : "+ Add Google account"}
        </button>
      </div>

      {/* Monitored Calendars */}
      <OAuthCalendarSelection
        groups={groups}
        onToggle={handleToggle}
        onRefresh={handleRefresh}
        isLoading={isLoading}
      />

      {/* Hidden-provider toggle */}
      <div className="border-border/60 flex items-center justify-between border-t pt-2.5">
        <div className="flex flex-col">
          <span className="text-xs font-medium">
            <Trans>Show in Calendar</Trans>
          </span>
          <span className="text-muted-foreground text-[11px]">
            <Trans>Display Google Calendar events on the main calendar view</Trans>
          </span>
        </div>
        <Switch
          checked={!isHidden}
          onCheckedChange={(checked) =>
            void setCalendarProviderHidden(config.id, !checked)
          }
          aria-label="Show in Calendar"
        />
      </div>

      <div className="flex items-center gap-3 pt-1">
        <button
          type="button"
          onClick={() => onDisconnect()}
          className="cursor-pointer text-xs text-destructive underline hover:text-destructive/80"
        >
          <Trans>Disconnect all</Trans>
        </button>
        <span className="text-muted-foreground text-xs">•</span>
        <button
          type="button"
          onClick={onReset}
          className="text-muted-foreground cursor-pointer text-xs underline hover:text-destructive"
        >
          <Trans>Disconnect &amp; remove client</Trans>
        </button>
      </div>
    </div>
  );
}

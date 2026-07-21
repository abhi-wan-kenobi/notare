import { Trans } from "@lingui/react/macro";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { open as selectFile } from "@tauri-apps/plugin-dialog";
import { useCallback, useMemo, useState } from "react";

import { commands as calendarCommands } from "@hypr/plugin-calendar";
import type { GoogleAccountStatus } from "@hypr/plugin-calendar";
import { commands as openerCommands } from "@hypr/plugin-opener2";

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
    mutationFn: async () => {
      const result = await calendarCommands.googleDisconnect();
      if (result.status === "error") throw new Error(result.error);
      return result.data;
    },
    onSuccess: () => {
      setActionError(null);
      void invalidateStatus();
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
        onDisconnect={() => disconnectMutation.mutate()}
        onReset={() => resetMutation.mutate()}
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
          <Trans>How do I get this file?</Trans>
        )}
      </button>
      {open && (
        <div className="border-border bg-muted/40 flex flex-col gap-2 rounded-md border p-2">
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
          <div className="flex items-center gap-2">
            <button
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
                onClick={() => void openerCommands.openUrl(docsPath, null)}
                className="text-muted-foreground hover:text-foreground cursor-pointer text-xs underline transition-colors"
              >
                <Trans>Full setup guide</Trans>
              </button>
            )}
          </div>
        </div>
      )}
    </div>
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
  onDisconnect,
  onReset,
}: {
  config: CalendarProvider;
  onDisconnect: () => void;
  onReset: () => void;
}) {
  const { groups, handleRefresh, handleToggle, isLoading } =
    useOAuthCalendarSelection(config);

  const groupsWithMenus = useMemo(
    () =>
      groups.map((group) => ({
        ...group,
        menuItems: [
          {
            id: "disconnect-google",
            text: "Disconnect",
            action: onDisconnect,
          },
          {
            id: "reset-google",
            text: "Disconnect & remove client",
            action: onReset,
          },
        ],
      })),
    [groups, onDisconnect, onReset],
  );

  return (
    <div className="flex flex-col gap-2">
      <OAuthCalendarSelection
        groups={groupsWithMenus}
        onToggle={handleToggle}
        onRefresh={handleRefresh}
        isLoading={isLoading}
      />
      <button
        onClick={onDisconnect}
        className="cursor-pointer self-start text-xs text-red-500 underline transition-colors hover:text-red-700"
      >
        Disconnect
      </button>
    </div>
  );
}

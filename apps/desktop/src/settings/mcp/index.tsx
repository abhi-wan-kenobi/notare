import { Trans, useLingui } from "@lingui/react/macro";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { CheckIcon, CopyIcon, RefreshCwIcon } from "lucide-react";
import { useState } from "react";

import {
  commands as mcpCommands,
  type McpServerSettings,
  type McpServerSettingsInput,
} from "@hypr/plugin-mcp-server";
import { Button } from "@hypr/ui/components/ui/button";
import { Input } from "@hypr/ui/components/ui/input";
import { Switch } from "@hypr/ui/components/ui/switch";
import { cn } from "@hypr/utils";

import { SettingsPageTitle } from "~/settings/page-title";

const MCP_SETTINGS_QUERY_KEY = ["settings", "mcp-server"] as const;
const MCP_STATUS_QUERY_KEY = ["settings", "mcp-server", "status"] as const;

function unwrap<T>(
  result: { status: "ok"; data: T } | { status: "error"; error: string },
): T {
  if (result.status === "error") {
    throw new Error(result.error);
  }
  return result.data;
}

export function SettingsMcp() {
  const {
    data: settings,
    isLoading,
    error,
  } = useQuery({
    queryKey: MCP_SETTINGS_QUERY_KEY,
    queryFn: async () => unwrap(await mcpCommands.getSettings()),
  });

  const { data: status } = useQuery({
    queryKey: MCP_STATUS_QUERY_KEY,
    queryFn: async () => unwrap(await mcpCommands.getStatus()),
    refetchInterval: 3000,
  });

  if (error) {
    throw error;
  }

  if (isLoading || !settings) {
    return (
      <div className="flex h-48 items-center justify-center">
        <span className="text-muted-foreground text-xs">
          <Trans>Loading MCP settings...</Trans>
        </span>
      </div>
    );
  }

  return (
    <McpSettingsContent
      settings={settings}
      isRunning={status?.running ?? false}
      errorMsg={status?.error}
    />
  );
}

function McpSettingsContent({
  settings,
  isRunning,
  errorMsg,
}: {
  settings: McpServerSettings;
  isRunning: boolean;
  errorMsg?: string | null;
}) {
  const { t } = useLingui();
  const queryClient = useQueryClient();

  const [enabled, setEnabled] = useState(settings.enabled);
  const [port, setPort] = useState(settings.port);
  const [tokenCopied, setTokenCopied] = useState(false);
  const [claudeCopied, setClaudeCopied] = useState(false);
  const [cursorCopied, setCursorCopied] = useState(false);

  const saveMutation = useMutation({
    mutationFn: async (next: McpServerSettingsInput) => {
      unwrap(await mcpCommands.setSettings(next));
    },
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: MCP_SETTINGS_QUERY_KEY });
      void queryClient.invalidateQueries({ queryKey: MCP_STATUS_QUERY_KEY });
    },
  });

  const regenerateMutation = useMutation({
    mutationFn: async () => {
      unwrap(await mcpCommands.regenerateToken());
    },
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: MCP_SETTINGS_QUERY_KEY });
    },
  });

  const handleToggle = (checked: boolean) => {
    setEnabled(checked);
    saveMutation.mutate({ enabled: checked, port: port ?? 8787 });
  };

  const handlePortBlur = () => {
    if (port !== settings.port) {
      saveMutation.mutate({ enabled: enabled ?? false, port: port ?? 8787 });
    }
  };

  const copyToClipboard = (text: string, setCopied: (v: boolean) => void) => {
    void navigator.clipboard.writeText(text);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  const claudeConfig = JSON.stringify(
    {
      mcpServers: {
        notare: {
          command: "notare-mcp",
          args: ["--port", String(port), "--token", settings.bearer_token],
        },
      },
    },
    null,
    2,
  );

  const cursorHttpConfig = JSON.stringify(
    {
      mcpServers: {
        notare: {
          url: `http://127.0.0.1:${port}`,
          headers: {
            Authorization: `Bearer ${settings.bearer_token}`,
          },
        },
      },
    },
    null,
    2,
  );

  return (
    <div className="flex flex-col gap-6">
      <SettingsPageTitle title={<Trans>Model Context Protocol (MCP)</Trans>} />

      <p className="text-muted-foreground text-xs leading-relaxed">
        <Trans>
          Expose a read-only streamable HTTP MCP server inside Notare on
          loopback (<code>127.0.0.1</code>) so external AI clients (like Claude
          Desktop, Cursor, or CLI agents) can query meetings, summaries, notes,
          transcripts, recipes, and daily notes.
        </Trans>
      </p>

      {/* Master Toggle */}
      <div className="bg-muted/40 flex items-center justify-between gap-4 rounded-lg border p-4">
        <div>
          <div className="flex items-center gap-2">
            <h3 className="text-sm font-medium">
              <Trans>Enable MCP Server</Trans>
            </h3>
            <span
              className={cn([
                "inline-flex items-center rounded-full px-2 py-0.5 text-[10px] font-semibold",
                isRunning
                  ? "bg-green-500/10 text-green-600 dark:text-green-400"
                  : "bg-muted text-muted-foreground",
              ])}
            >
              {isRunning ? t`Running on port ${port}` : t`Stopped`}
            </span>
          </div>
          <p className="text-muted-foreground mt-1 text-xs">
            <Trans>
              Listens on loopback only. Requires bearer token authentication.
            </Trans>
          </p>
          {errorMsg && (
            <p className="text-destructive mt-1 text-xs font-medium">
              {errorMsg}
            </p>
          )}
        </div>
        <Switch checked={enabled} onCheckedChange={handleToggle} />
      </div>

      {/* Port Configuration */}
      <div className="flex flex-col gap-2">
        <label className="text-sm font-medium">
          <Trans>Server Port</Trans>
        </label>
        <div className="flex max-w-xs items-center gap-2">
          <Input
            type="number"
            min={1024}
            max={65535}
            value={port}
            onChange={(e) => setPort(Number(e.target.value))}
            onBlur={handlePortBlur}
            disabled={saveMutation.isPending}
            className="w-32"
          />
          <span className="text-muted-foreground text-xs">
            <Trans>(default: 8787)</Trans>
          </span>
        </div>
      </div>

      {/* Bearer Token */}
      <div className="flex flex-col gap-2">
        <label className="text-sm font-medium">
          <Trans>Bearer Token</Trans>
        </label>
        <p className="text-muted-foreground text-xs">
          <Trans>
            Per-install secret token stored securely in your OS keychain.
            Required in the <code>Authorization: Bearer &lt;token&gt;</code>{" "}
            header.
          </Trans>
        </p>
        <div className="flex max-w-xl items-center gap-2">
          <Input
            type="password"
            readOnly
            value={settings.bearer_token}
            className="font-mono text-xs"
          />
          <Button
            variant="outline"
            size="sm"
            onClick={() =>
              copyToClipboard(settings.bearer_token, setTokenCopied)
            }
          >
            {tokenCopied ? (
              <CheckIcon className="h-4 w-4 text-green-500" />
            ) : (
              <CopyIcon className="h-4 w-4" />
            )}
            <span className="ml-1">{tokenCopied ? t`Copied` : t`Copy`}</span>
          </Button>
          <Button
            variant="ghost"
            size="sm"
            disabled={regenerateMutation.isPending}
            onClick={() => regenerateMutation.mutate()}
          >
            <RefreshCwIcon
              className={cn([
                "h-4 w-4",
                regenerateMutation.isPending && "animate-spin",
              ])}
            />
            <span className="ml-1">{t`Regenerate`}</span>
          </Button>
        </div>
      </div>

      {/* Client Configuration Snippets */}
      <div className="flex flex-col gap-4 border-t pt-4">
        <h4 className="text-sm font-medium">
          <Trans>Client Configuration Snippets</Trans>
        </h4>

        {/* Claude Desktop */}
        <div className="flex flex-col gap-2">
          <div className="flex items-center justify-between">
            <span className="text-xs font-semibold">
              <Trans>Claude Desktop (stdio bridge: notare-mcp)</Trans>
            </span>
            <Button
              variant="outline"
              size="sm"
              onClick={() => copyToClipboard(claudeConfig, setClaudeCopied)}
            >
              {claudeCopied ? (
                <CheckIcon className="h-3 w-3 text-green-500" />
              ) : (
                <CopyIcon className="h-3 w-3" />
              )}
              <span className="ml-1 text-[11px]">
                {claudeCopied ? t`Copied` : t`Copy JSON`}
              </span>
            </Button>
          </div>
          <pre className="bg-muted/60 text-muted-foreground overflow-x-auto rounded p-3 font-mono text-[11px] leading-relaxed">
            {claudeConfig}
          </pre>
        </div>

        {/* Cursor */}
        <div className="flex flex-col gap-2">
          <div className="flex items-center justify-between">
            <span className="text-xs font-semibold">
              <Trans>Cursor (HTTP SSE transport)</Trans>
            </span>
            <Button
              variant="outline"
              size="sm"
              onClick={() => copyToClipboard(cursorHttpConfig, setCursorCopied)}
            >
              {cursorCopied ? (
                <CheckIcon className="h-3 w-3 text-green-500" />
              ) : (
                <CopyIcon className="h-3 w-3" />
              )}
              <span className="ml-1 text-[11px]">
                {cursorCopied ? t`Copied` : t`Copy JSON`}
              </span>
            </Button>
          </div>
          <pre className="bg-muted/60 text-muted-foreground overflow-x-auto rounded p-3 font-mono text-[11px] leading-relaxed">
            {cursorHttpConfig}
          </pre>
        </div>
      </div>
    </div>
  );
}

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  AlertCircleIcon,
  CheckCircle2Icon,
  RefreshCwIcon,
  WrenchIcon,
} from "lucide-react";

import {
  type ProviderHealth,
  type ProviderKind,
  commands as agentCommands,
} from "@hypr/plugin-agent";
import { Button } from "@hypr/ui/components/ui/button";
import { sonnerToast } from "@hypr/ui/components/ui/toast";
import { cn } from "@hypr/utils";

const DEFAULT_PROVIDER_HEALTH: ProviderHealth[] = [
  {
    provider: "claude",
    binaryPath: "claude",
    installed: false,
    integrationInstalled: false,
    version: null,
    status: "warning",
    authStatus: "unknown",
    message: "Checking Claude Code CLI status...",
  },
  {
    provider: "codex",
    binaryPath: "codex",
    installed: false,
    integrationInstalled: false,
    version: null,
    status: "warning",
    authStatus: "unknown",
    message: "Checking Codex CLI status...",
  },
  {
    provider: "opencode",
    binaryPath: "opencode",
    installed: false,
    integrationInstalled: false,
    version: null,
    status: "warning",
    authStatus: "unknown",
    message: "Checking OpenCode CLI status...",
  },
];

function providerLabel(provider: ProviderKind) {
  switch (provider) {
    case "claude":
      return "Claude Code";
    case "codex":
      return "Codex";
    case "opencode":
      return "OpenCode";
  }
}

function formatAuthStatus(status: ProviderHealth["authStatus"]) {
  switch (status) {
    case "authenticated":
      return "Authenticated";
    case "unauthenticated":
      return "Unauthenticated";
    case "unknown":
      return "Unknown";
  }
}

function describeProvider(provider: ProviderHealth) {
  if (provider.message) {
    return provider.message;
  }

  if (!provider.installed) {
    return `${providerLabel(provider.provider)} CLI is not installed.`;
  }

  return `${providerLabel(provider.provider)} CLI is ready for integration.`;
}

function AgentIntegrationCard({
  provider,
  onInstalled,
}: {
  provider: ProviderHealth;
  onInstalled: () => Promise<void>;
}) {
  const installMutation = useMutation({
    mutationFn: async () => {
      const result = await agentCommands.installCli({
        provider: provider.provider,
      });
      if (result.status === "error") {
        throw new Error(result.error);
      }
      return result.data;
    },
    onSuccess: async (data) => {
      sonnerToast.success(data.message);
      await onInstalled();
    },
    onError: (error) => {
      sonnerToast.error(error.message);
    },
  });

  const tone =
    provider.status === "ready"
      ? {
          border: "border-emerald-200",
          bg: "bg-emerald-50/70",
          icon: "text-emerald-600",
          badge: "bg-emerald-100 text-emerald-800",
        }
      : provider.status === "warning"
        ? {
            border: "border-amber-200",
            bg: "bg-amber-50/70",
            icon: "text-amber-600",
            badge: "bg-amber-100 text-amber-800",
          }
        : {
            border: "border-red-200",
            bg: "bg-red-50/70",
            icon: "text-red-600",
            badge: "bg-red-100 text-red-800",
          };

  return (
    <div
      className={cn([
        "flex items-center gap-4 rounded-xl border px-4 py-3",
        tone.border,
        tone.bg,
      ])}
    >
      <div className="min-w-0 flex-1">
        <div className="mb-1 flex items-center gap-2">
          {provider.status === "ready" ? (
            <CheckCircle2Icon className={cn(["h-4 w-4 shrink-0", tone.icon])} />
          ) : (
            <AlertCircleIcon className={cn(["h-4 w-4 shrink-0", tone.icon])} />
          )}
          <span className="text-sm font-medium">
            {providerLabel(provider.provider)}
          </span>
          <span
            className={cn([
              "rounded-full px-2 py-0.5 text-[11px] font-medium capitalize",
              tone.badge,
            ])}
          >
            {provider.status}
          </span>
        </div>
        <p className="text-xs text-neutral-700">{describeProvider(provider)}</p>
        <div className="mt-1 flex flex-wrap gap-x-3 gap-y-1 text-[11px] text-neutral-500">
          <span>Binary: {provider.binaryPath}</span>
          <span>Version: {provider.version ?? "Unknown"}</span>
          <span>Auth: {formatAuthStatus(provider.authStatus)}</span>
          <span>
            Hook:{" "}
            {provider.integrationInstalled ? "Installed" : "Not installed"}
          </span>
        </div>
      </div>
      <Button
        size="sm"
        variant="outline"
        onClick={() => installMutation.mutate()}
        disabled={installMutation.isPending}
        type="button"
      >
        <WrenchIcon className="mr-2 h-4 w-4" />
        {installMutation.isPending
          ? "Installing..."
          : provider.integrationInstalled
            ? "Reinstall"
            : "Install"}
      </Button>
    </div>
  );
}

export function AgentIntegrations() {
  const queryClient = useQueryClient();
  const healthQuery = useQuery({
    queryKey: ["agent-integrations-health"],
    queryFn: async () => {
      const result = await agentCommands.healthCheck();
      if (result.status === "error") {
        throw new Error(result.error);
      }
      return result.data.providers;
    },
    refetchInterval: 30_000,
  });

  const refresh = async () => {
    await queryClient.invalidateQueries({
      queryKey: ["agent-integrations-health"],
    });
  };

  return (
    <section className="flex flex-col gap-3">
      <div className="flex items-center justify-between gap-4">
        <div className="flex-1">
          <h3 className="mb-1 text-sm font-medium">Agent Integrations</h3>
          <p className="text-xs text-neutral-600">
            Check Codex, Claude Code, and OpenCode CLI status and install Char
            hooks.
          </p>
        </div>
        <Button
          size="sm"
          variant="outline"
          onClick={() => void refresh()}
          disabled={healthQuery.isFetching}
        >
          <RefreshCwIcon
            className={cn([
              "mr-2 h-4 w-4",
              healthQuery.isFetching && "animate-spin",
            ])}
          />
          Refresh
        </Button>
      </div>

      {healthQuery.isError ? (
        <div className="rounded-xl border border-red-200 bg-red-50 px-4 py-3 text-sm text-red-800">
          Failed to load agent integration status.
        </div>
      ) : null}

      <div className="grid gap-3">
        {(healthQuery.data ?? DEFAULT_PROVIDER_HEALTH).map((provider) => (
          <AgentIntegrationCard
            key={provider.provider}
            provider={provider}
            onInstalled={refresh}
          />
        ))}
      </div>
    </section>
  );
}

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  AlertCircleIcon,
  CheckCircle2Icon,
  RefreshCwIcon,
  TerminalIcon,
  Trash2Icon,
  WrenchIcon,
} from "lucide-react";

import { Badge } from "@hypr/ui/components/ui/badge";
import { Button } from "@hypr/ui/components/ui/button";
import { sonnerToast } from "@hypr/ui/components/ui/toast";
import { cn } from "@hypr/utils";

import { commands, type EmbeddedCliStatus } from "~/types/tauri.gen";

function statusCopy(status: EmbeddedCliStatus) {
  switch (status.state) {
    case "installed":
      return "Installed and managed by this app.";
    case "missing":
      return "Not installed yet.";
    case "conflict":
      return "A different command already exists at this path.";
    case "resource_missing":
      return "This build does not include the embedded CLI resource.";
    case "unsupported":
      return "Embedded CLI install is only supported on macOS.";
  }
}

function statusBadge(status: EmbeddedCliStatus) {
  switch (status.state) {
    case "installed":
      return {
        label: "Installed",
        className: "bg-emerald-50 text-emerald-700",
        icon: <CheckCircle2Icon className="size-3" />,
      };
    case "missing":
      return {
        label: "Missing",
        className: "bg-neutral-100 text-neutral-600",
        icon: <TerminalIcon className="size-3" />,
      };
    case "conflict":
      return {
        label: "Conflict",
        className: "bg-amber-50 text-amber-700",
        icon: <AlertCircleIcon className="size-3" />,
      };
    case "resource_missing":
      return {
        label: "Resource Missing",
        className: "bg-amber-50 text-amber-700",
        icon: <AlertCircleIcon className="size-3" />,
      };
    case "unsupported":
      return {
        label: "Unsupported",
        className: "bg-neutral-100 text-neutral-600",
        icon: <AlertCircleIcon className="size-3" />,
      };
  }
}

async function loadStatus() {
  const result = await commands.checkEmbeddedCli();
  if (result.status === "error") {
    throw new Error(result.error);
  }
  return result.data;
}

export function CommandLineSettings() {
  const queryClient = useQueryClient();
  const query = useQuery({
    queryKey: ["embedded-cli-status"],
    queryFn: loadStatus,
  });

  const installMutation = useMutation({
    mutationFn: async () => {
      const result = await commands.installEmbeddedCli();
      if (result.status === "error") {
        throw new Error(result.error);
      }
      return result.data;
    },
    onSuccess: (data) => {
      queryClient.setQueryData(["embedded-cli-status"], data);
      sonnerToast.success(`Installed ${data.commandName}`);
    },
    onError: (error) => {
      sonnerToast.error(error.message);
    },
  });

  const uninstallMutation = useMutation({
    mutationFn: async () => {
      const result = await commands.uninstallEmbeddedCli();
      if (result.status === "error") {
        throw new Error(result.error);
      }
      return result.data;
    },
    onSuccess: (data) => {
      queryClient.setQueryData(["embedded-cli-status"], data);
      sonnerToast.success(`Removed ${data.commandName}`);
    },
    onError: (error) => {
      sonnerToast.error(error.message);
    },
  });

  const status = query.data;
  const badge = status ? statusBadge(status) : null;
  const isBusy = installMutation.isPending || uninstallMutation.isPending;
  const installLabel =
    status?.state === "installed"
      ? "Reinstall"
      : status?.state === "conflict"
        ? "Replace"
        : "Install";
  const installPendingLabel =
    status?.state === "installed"
      ? "Reinstalling..."
      : status?.state === "conflict"
        ? "Replacing..."
        : "Installing...";

  return (
    <section className="flex flex-col gap-3">
      <div className="flex items-center justify-between gap-4">
        <div className="flex-1">
          <h2 className="mb-1 font-serif text-lg font-semibold">
            Command Line
          </h2>
          <p className="text-xs text-neutral-600">
            Install the embedded Char CLI as a shell command.
          </p>
        </div>
        <Button
          size="sm"
          variant="ghost"
          onClick={() =>
            void queryClient.invalidateQueries({
              queryKey: ["embedded-cli-status"],
            })
          }
          disabled={query.isFetching || isBusy}
        >
          <RefreshCwIcon
            className={cn(["size-4", query.isFetching && "animate-spin"])}
          />
        </Button>
      </div>

      {query.isError ? (
        <p className="text-xs text-red-600">
          Failed to load embedded CLI status.
        </p>
      ) : query.isPending || !status ? (
        <p className="text-xs text-neutral-500">Checking command status...</p>
      ) : (
        <div className="rounded-xl border border-neutral-200 bg-white p-4">
          <div className="flex flex-wrap items-center gap-2">
            <span className="text-sm font-medium">{status.commandName}</span>
            <Badge
              variant="secondary"
              className={cn([
                "flex items-center gap-1 border-0 px-2 py-0.5 text-[10px]",
                badge?.className ?? "",
              ])}
            >
              {badge?.icon}
              {badge?.label}
            </Badge>
          </div>

          <p className="mt-2 text-xs text-neutral-600">
            {status.details ?? statusCopy(status)}
          </p>

          <dl className="mt-4 space-y-3 text-xs">
            <div className="grid gap-1">
              <dt className="font-medium text-neutral-500">Install Path</dt>
              <dd className="font-mono text-neutral-800">
                {status.installPath}
              </dd>
            </div>
            <div className="grid gap-1">
              <dt className="font-medium text-neutral-500">
                Embedded Resource
              </dt>
              <dd className="font-mono text-neutral-800">
                {status.resourcePath ?? "Not available"}
              </dd>
            </div>
          </dl>

          <div className="mt-4 flex flex-wrap gap-2">
            <Button
              size="sm"
              type="button"
              onClick={() => installMutation.mutate()}
              disabled={
                isBusy ||
                status.state === "unsupported" ||
                status.state === "resource_missing"
              }
            >
              <WrenchIcon className="mr-2 size-4" />
              {installMutation.isPending ? installPendingLabel : installLabel}
            </Button>
            <Button
              size="sm"
              type="button"
              variant="outline"
              onClick={() => uninstallMutation.mutate()}
              disabled={isBusy || status.state !== "installed"}
            >
              <Trash2Icon className="mr-2 size-4" />
              {uninstallMutation.isPending ? "Uninstalling..." : "Uninstall"}
            </Button>
          </div>
        </div>
      )}
    </section>
  );
}

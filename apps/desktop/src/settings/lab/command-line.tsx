import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Trash2Icon, WrenchIcon } from "lucide-react";

import { Button } from "@hypr/ui/components/ui/button";
import { sonnerToast } from "@hypr/ui/components/ui/toast";

import { commands } from "~/types/tauri.gen";

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
  const isBusy = installMutation.isPending || uninstallMutation.isPending;

  return (
    <section className="flex flex-col gap-3">
      <div>
        <h2 className="mb-1 font-serif text-lg font-semibold">Command Line</h2>
        <p className="text-xs text-neutral-600">
          Install the embedded Char CLI as a shell command.
        </p>
      </div>

      {query.isError ? (
        <p className="text-xs text-red-600">Failed to load CLI status.</p>
      ) : query.isPending || !status ? (
        <p className="text-xs text-neutral-500">Checking...</p>
      ) : (
        <div className="flex items-center justify-between">
          <div>
            <p className="text-sm font-medium">{status.commandName}</p>
            <p className="text-xs text-neutral-500">{status.installPath}</p>
          </div>

          {status.state === "installed" ? (
            <Button
              size="sm"
              type="button"
              variant="outline"
              onClick={() => uninstallMutation.mutate()}
              disabled={isBusy}
            >
              <Trash2Icon className="mr-2 size-3.5" />
              {uninstallMutation.isPending ? "Uninstalling..." : "Uninstall"}
            </Button>
          ) : (
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
              <WrenchIcon className="mr-2 size-3.5" />
              {installMutation.isPending
                ? status.state === "conflict"
                  ? "Replacing..."
                  : "Installing..."
                : status.state === "conflict"
                  ? "Replace"
                  : "Install"}
            </Button>
          )}
        </div>
      )}
    </section>
  );
}

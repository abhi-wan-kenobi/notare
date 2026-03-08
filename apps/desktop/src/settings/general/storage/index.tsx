import { useQuery, useQueryClient } from "@tanstack/react-query";
import { homeDir } from "@tauri-apps/api/path";
import {
  ArrowDownIcon,
  FolderIcon,
  type LucideIcon,
  Settings2Icon,
} from "lucide-react";
import { type ReactNode, useEffect } from "react";
import { useState } from "react";

import { commands as fsSyncCommands } from "@hypr/plugin-fs-sync";
import { commands as openerCommands } from "@hypr/plugin-opener2";
import { commands as settingsCommands } from "@hypr/plugin-settings";
import { Button } from "@hypr/ui/components/ui/button";
import { Checkbox } from "@hypr/ui/components/ui/checkbox";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@hypr/ui/components/ui/dialog";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@hypr/ui/components/ui/tooltip";
import { cn } from "@hypr/utils";

import { useChangeContentPathWizard } from "./use-storage-wizard";

import * as main from "~/store/tinybase/store/main";

function tildify(path: string, home: string) {
  return path.startsWith(home + "/") ? "~" + path.slice(home.length) : path;
}

function shortenPath(path: string, maxLength = 48): string {
  if (path.length <= maxLength) return path;
  const short = path.slice(path.length - maxLength);
  const slash = short.indexOf("/");
  return "\u2026" + (slash > 0 ? short.slice(slash) : short);
}

export function StorageSettingsView() {
  const queryClient = useQueryClient();
  const { data: home } = useQuery({ queryKey: ["home-dir"], queryFn: homeDir });
  const { data: othersBase } = useQuery({
    queryKey: ["others-base-path"],
    queryFn: async () => {
      const result = await settingsCommands.globalBase();
      if (result.status === "error") {
        throw new Error(result.error);
      }
      return result.data;
    },
  });

  const { data: contentBase } = useQuery({
    queryKey: ["content-base-path"],
    queryFn: async () => {
      const result = await settingsCommands.vaultBase();
      if (result.status === "error") {
        throw new Error(result.error);
      }
      return result.data;
    },
  });
  const [showDialog, setShowDialog] = useState(false);

  return (
    <div>
      <h2 className="mb-4 font-serif text-lg font-semibold">Storage</h2>
      <div className="flex flex-col gap-3">
        <StoragePathRow
          icon={FolderIcon}
          title="Content"
          description="Stores your notes, recordings, and session data"
          path={contentBase}
          home={home}
          action={
            <Button
              variant="outline"
              size="sm"
              onClick={() => setShowDialog(true)}
              disabled={!contentBase}
            >
              Customize
            </Button>
          }
        />
        <StoragePathRow
          icon={Settings2Icon}
          title="Others"
          description="Stores app-wide settings and configurations"
          path={othersBase}
          home={home}
        />
      </div>
      <ChangeContentPathDialog
        open={showDialog}
        currentPath={contentBase}
        home={home}
        onOpenChange={setShowDialog}
        onSuccess={() => {
          void queryClient.invalidateQueries({
            queryKey: ["content-base-path"],
          });
        }}
      />
    </div>
  );
}

function ChangeContentPathDialog({
  open,
  currentPath,
  home,
  onOpenChange,
  onSuccess,
}: {
  open: boolean;
  currentPath: string | undefined;
  home: string | undefined;
  onOpenChange: (open: boolean) => void;
  onSuccess: () => void;
}) {
  const {
    selectedPath,
    copyVault,
    setCopyVault,
    chooseFolder,
    apply,
    isPending,
    error,
  } = useChangeContentPathWizard({ open, currentPath, onSuccess });

  const currentSessionCount = main.UI.useRowIds(
    "sessions",
    main.STORE_ID,
  ).length;

  const isNewPathChosen = !!selectedPath && selectedPath !== currentPath;

  const { data: isNewPathEmpty, isLoading: isCheckingNewPath } = useQuery({
    queryKey: ["path-empty-check", selectedPath],
    enabled: isNewPathChosen,
    queryFn: async () => {
      const result = await fsSyncCommands.scanAndRead(
        selectedPath!,
        ["*"],
        false,
        null,
      );
      if (result.status === "error") return true; // dir doesn't exist yet → trivially empty, Rust will create it
      return (
        Object.keys(result.data.files).length === 0 &&
        result.data.dirs.length === 0
      );
    },
  });

  useEffect(() => {
    if (isNewPathEmpty !== undefined) {
      setCopyVault(isNewPathEmpty);
    }
  }, [isNewPathEmpty, setCopyVault]);

  const disabledReason = (() => {
    if (!selectedPath || selectedPath === currentPath)
      return "Select a different folder";
    if (isCheckingNewPath) return "Checking folder...";
    return null;
  })();

  return (
    <Dialog
      open={open}
      onOpenChange={(nextOpen) => {
        if (isPending) return;
        onOpenChange(nextOpen);
      }}
    >
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Change content location</DialogTitle>
          <DialogDescription>
            Choose where Char should store data. (notes, settings, etc)
          </DialogDescription>
        </DialogHeader>

        <div className="flex flex-col">
          <PathBox
            label="Current"
            path={
              currentPath && home
                ? tildify(currentPath, home)
                : (currentPath ?? "Loading...")
            }
            sessionCount={currentSessionCount}
          />
          <div className="flex justify-center py-2 text-neutral-400">
            <ArrowDownIcon className="size-4" />
          </div>
          <div className="rounded-lg border border-neutral-200 bg-neutral-50 px-3 py-2">
            <div className="flex items-center gap-3">
              <div className="min-w-0 flex-1">
                <p className="text-xs font-medium tracking-wide text-neutral-500 uppercase">
                  New
                </p>
                <p
                  className={cn([
                    "mt-1 text-sm",
                    selectedPath && selectedPath !== currentPath
                      ? "text-neutral-700"
                      : "text-neutral-400",
                  ])}
                >
                  {selectedPath && home
                    ? shortenPath(tildify(selectedPath, home))
                    : selectedPath
                      ? shortenPath(selectedPath)
                      : "Select a folder"}
                </p>
                {isNewPathChosen && isNewPathEmpty !== undefined && (
                  <p className="mt-1 text-xs text-neutral-400">
                    {isNewPathEmpty ? "Empty folder" : "Not empty"}
                  </p>
                )}
              </div>
              <Button
                variant="outline"
                size="sm"
                className="shrink-0"
                onClick={chooseFolder}
              >
                Choose
              </Button>
            </div>
          </div>
        </div>

        {error && <p className="text-sm text-red-500">{error.message}</p>}

        {isNewPathChosen && (
          <DialogFooter className="items-center">
            {!disabledReason && (
              <label className="mr-auto flex cursor-pointer items-center gap-2">
                <Checkbox
                  checked={copyVault}
                  onCheckedChange={(v) => setCopyVault(v === true)}
                />
                <div className="flex flex-row gap-1">
                  <span className="text-sm font-semibold text-neutral-600">
                    Copy
                  </span>
                  <span className="text-sm text-neutral-600">
                    existing data to new location
                  </span>
                </div>
              </label>
            )}
            <Tooltip>
              <TooltipTrigger asChild>
                <span
                  className={cn([
                    disabledReason ? "cursor-not-allowed" : "cursor-pointer",
                  ])}
                >
                  <Button
                    onClick={apply}
                    disabled={!!disabledReason || isPending}
                    className={cn([
                      disabledReason ? "pointer-events-none" : "",
                    ])}
                  >
                    {isPending ? "Applying..." : "Apply and Restart"}
                  </Button>
                </span>
              </TooltipTrigger>
              {disabledReason && (
                <TooltipContent>
                  <p className="text-xs">{disabledReason}</p>
                </TooltipContent>
              )}
            </Tooltip>
          </DialogFooter>
        )}
      </DialogContent>
    </Dialog>
  );
}

function PathBox({
  label,
  path,
  sessionCount,
}: {
  label: string;
  path: string;
  sessionCount: number;
}) {
  return (
    <div className="rounded-lg border border-neutral-200 bg-neutral-50 px-3 py-2">
      <p className="text-xs font-medium tracking-wide text-neutral-500 uppercase">
        {label}
      </p>
      <p className="mt-1 text-sm text-neutral-700">{shortenPath(path)}</p>
      <p className="mt-1 text-xs text-neutral-400">
        {sessionCount === 0
          ? "No sessions"
          : `${sessionCount} session${sessionCount === 1 ? "" : "s"}`}
      </p>
    </div>
  );
}

function StoragePathRow({
  icon: Icon,
  title,
  description,
  path,
  home,
  action,
}: {
  icon: LucideIcon;
  title: string;
  description: string;
  path: string | undefined;
  home: string | undefined;
  action?: ReactNode;
}) {
  return (
    <div className="flex items-center gap-3">
      <Tooltip delayDuration={0}>
        <TooltipTrigger asChild>
          <div className="flex w-24 shrink-0 cursor-default items-center gap-2">
            <Icon className="size-4 text-neutral-500" />
            <span className="text-sm font-medium">{title}</span>
          </div>
        </TooltipTrigger>
        <TooltipContent side="top">
          <p className="text-xs">{description}</p>
        </TooltipContent>
      </Tooltip>
      <button
        onClick={() => path && openerCommands.openPath(path, null)}
        className="min-w-0 flex-1 cursor-pointer truncate text-left text-sm text-neutral-500 hover:underline"
      >
        {path && home
          ? shortenPath(tildify(path, home))
          : path
            ? shortenPath(path)
            : "Loading..."}
      </button>
      {action && <div className="shrink-0">{action}</div>}
    </div>
  );
}

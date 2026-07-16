import { Trans, useLingui } from "@lingui/react/macro";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { homeDir } from "@tauri-apps/api/path";
import { message, open as selectFolder } from "@tauri-apps/plugin-dialog";
import { FolderIcon } from "lucide-react";
import { useState } from "react";

import { commands as openerCommands } from "@hypr/plugin-opener2";
import { commands as settingsCommands } from "@hypr/plugin-settings";
import { Button } from "@hypr/ui/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@hypr/ui/components/ui/dialog";

import { ObsidianVaultList } from "./obsidian-vault-list";
import { displayPath } from "./path-utils";

import { scheduleAutomaticRelaunch } from "~/shared/relaunch";

export type NotesMigrationStrategy = "move" | "copy" | "switch";

export function resolveMigrationStrategy(
  migrate: boolean,
  targetEmptyOrMissing: boolean,
): NotesMigrationStrategy {
  if (!migrate) {
    return "switch";
  }
  return targetEmptyOrMissing ? "move" : "copy";
}

// "move"   = copy everything to the new folder, point the app there, then
//            clean up the old copies (settings plugin `move_vault`; it never
//            renames across devices, so cross-drive moves are safe).
// "copy"   = destination already has files (e.g. an Obsidian vault), so the
//            notes are copied and the originals are left in place.
// "switch" = only re-point the app; existing files stay where they are.
export async function changeNotesLocation(
  newPath: string,
  migrate: boolean,
): Promise<NotesMigrationStrategy> {
  let strategy: NotesMigrationStrategy = "switch";

  if (migrate) {
    const emptyResult = await settingsCommands.isEmptyOrMissingDir(newPath);
    strategy = resolveMigrationStrategy(
      true,
      emptyResult.status === "ok" && emptyResult.data,
    );
  }

  if (strategy === "move") {
    const result = await settingsCommands.moveVault(newPath);
    if (result.status === "error") {
      throw new Error(result.error);
    }
    return strategy;
  }

  if (strategy === "copy") {
    const copyResult = await settingsCommands.copyVault(newPath);
    if (copyResult.status === "error") {
      throw new Error(copyResult.error);
    }
  }

  const setResult = await settingsCommands.setVaultBase(newPath);
  if (setResult.status === "error") {
    throw new Error(setResult.error);
  }

  return strategy;
}

export function NotesLocationSection() {
  const { t } = useLingui();
  const queryClient = useQueryClient();
  const [pendingPath, setPendingPath] = useState<string | null>(null);

  const { data: home } = useQuery({ queryKey: ["home-dir"], queryFn: homeDir });
  const { data: vaultBase } = useQuery({
    queryKey: ["vault-base-path"],
    queryFn: async () => {
      const result = await settingsCommands.vaultBase();
      if (result.status === "error") {
        throw new Error(result.error);
      }
      return result.data;
    },
  });

  const { data: obsidianVaults } = useQuery({
    queryKey: ["obsidian-vaults"],
    queryFn: async () => {
      const result = await settingsCommands.obsidianVaults();
      if (result.status === "error") return [];
      return result.data;
    },
  });

  const changeMutation = useMutation({
    mutationFn: async ({
      newPath,
      migrate,
    }: {
      newPath: string;
      migrate: boolean;
    }) => changeNotesLocation(newPath, migrate),
    onSuccess: async (strategy) => {
      setPendingPath(null);
      queryClient.invalidateQueries({ queryKey: ["vault-base-path"] });

      if (strategy === "copy") {
        await message(
          t`The new folder already contained files, so your notes were copied there. The originals were left at the old location.`,
          { kind: "info", title: t`Notes copied` },
        );
      }

      const restartStatus = await scheduleAutomaticRelaunch();
      if (restartStatus === "deferred") {
        void message(
          t`The app will restart after onboarding to apply your storage changes`,
          { kind: "info", title: t`Storage Updated` },
        );
      }
    },
  });

  const handleChoose = async () => {
    const selected = await selectFolder({
      title: t`Choose notes folder`,
      directory: true,
      multiple: false,
      defaultPath: vaultBase ?? undefined,
    });

    if (typeof selected === "string" && selected && selected !== vaultBase) {
      setPendingPath(selected);
    }
  };

  const handleOpenPath = () => {
    if (vaultBase) {
      openerCommands.openPath(vaultBase, null);
    }
  };

  const detectedVaults = (obsidianVaults ?? []).filter(
    (vault) => vault.path !== vaultBase,
  );

  return (
    <div className="flex flex-col gap-2">
      <div className="flex min-w-0 items-center gap-2">
        <FolderIcon className="text-muted-foreground size-4 shrink-0" />
        <span className="truncate text-sm font-medium">
          <Trans>Notes folder</Trans>
        </span>
      </div>
      <p className="text-muted-foreground text-xs">
        <Trans>
          Your notes, transcripts, and recordings are saved here as plain files.
          Pick an Obsidian vault or any other folder.
        </Trans>
      </p>
      <div className="border-border bg-muted flex items-center gap-3 rounded-lg border px-4 py-3">
        <button
          onClick={handleOpenPath}
          className="text-muted-foreground min-w-0 flex-1 truncate text-left text-sm hover:underline"
        >
          {displayPath(vaultBase, home)}
        </button>
        <button
          onClick={handleChoose}
          disabled={changeMutation.isPending}
          className="text-muted-foreground hover:text-foreground shrink-0 text-sm transition-colors disabled:opacity-50"
        >
          <Trans>Change</Trans>
        </button>
      </div>

      <ObsidianVaultList
        vaults={detectedVaults}
        home={home}
        disabled={changeMutation.isPending}
        onSelect={(path) => {
          if (path !== vaultBase) {
            setPendingPath(path);
          }
        }}
        actionLabel={t`Use`}
      />

      {changeMutation.error && (
        <p className="text-xs text-red-500">{changeMutation.error.message}</p>
      )}

      <Dialog
        open={pendingPath !== null}
        onOpenChange={(open) => {
          if (!changeMutation.isPending && !open) {
            setPendingPath(null);
          }
        }}
      >
        <DialogContent className="border-border/45 bg-card/95 w-[calc(100vw-48px)] max-w-[360px] gap-0 overflow-hidden rounded-[26px] p-0 shadow-[0_24px_70px_rgba(0,0,0,0.32)] backdrop-blur-xl sm:rounded-[26px] [&>button:last-child]:hidden">
          <DialogHeader className="items-center gap-2 px-5 pt-6 text-center sm:text-center">
            <DialogTitle className="text-foreground text-[13px] leading-5 font-semibold tracking-normal">
              <Trans>Move existing notes?</Trans>
            </DialogTitle>
            <DialogDescription className="text-foreground w-full text-center text-[13px] leading-[1.36]">
              <Trans>
                New notes will be saved to{" "}
                {displayPath(pendingPath ?? undefined, home)}. You can also move
                your existing notes there. If the folder already contains files,
                your notes are copied and the originals stay in place. The app
                will restart to apply the change.
              </Trans>
            </DialogDescription>
          </DialogHeader>

          <DialogFooter className="grid grid-cols-3 gap-2 px-4 pt-4 pb-4 sm:grid-cols-3 sm:justify-normal">
            <Button
              variant="ghost"
              className="bg-accent/80 text-foreground hover:bg-accent hover:text-foreground h-8 rounded-full px-3 text-xs font-medium shadow-none"
              onClick={() => setPendingPath(null)}
              disabled={changeMutation.isPending}
            >
              <Trans>Cancel</Trans>
            </Button>
            <Button
              variant="ghost"
              className="bg-accent/80 text-foreground hover:bg-accent hover:text-foreground h-8 rounded-full px-3 text-xs font-medium shadow-none"
              onClick={() =>
                pendingPath &&
                changeMutation.mutate({ newPath: pendingPath, migrate: false })
              }
              disabled={changeMutation.isPending}
            >
              <Trans>Don't move</Trans>
            </Button>
            <Button
              className="h-8 rounded-full px-3 text-xs font-medium shadow-sm"
              onClick={() =>
                pendingPath &&
                changeMutation.mutate({ newPath: pendingPath, migrate: true })
              }
              disabled={changeMutation.isPending}
            >
              {changeMutation.isPending ? t`Moving...` : t`Move notes`}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}

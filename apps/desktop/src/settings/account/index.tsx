import { Trans, useLingui } from "@lingui/react/macro";
import { useQuery } from "@tanstack/react-query";
import { homeDir } from "@tauri-apps/api/path";
import { message } from "@tauri-apps/plugin-dialog";
import {
  DownloadIcon,
  FolderIcon,
  Trash2Icon,
  TriangleAlertIcon,
} from "lucide-react";
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

import { SectionLabel, SettingsRow } from "~/settings/components";
import { StorageSettingsView } from "~/settings/general/storage";
import { displayPath } from "~/settings/general/storage/path-utils";

export function AccountSettingsView() {
  const { t } = useLingui();
  const [resetModalOpen, setResetModalOpen] = useState(false);

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

  const handleRevealVault = () => {
    if (vaultBase) {
      void openerCommands.openPath(vaultBase, null);
    }
  };

  const handleExportNotes = () => {
    // Reveal the notes directory where all Markdown files and attachments are stored
    if (vaultBase) {
      void openerCommands.openPath(vaultBase, null);
    }
  };

  const handleConfirmReset = async () => {
    setResetModalOpen(false);
    await message(
      t`To complete resetting local data, quit Notare and clear the data directory, or delete the app support folder.`,
      { kind: "info", title: t`Reset Local Data` },
    );
  };

  const formattedVaultPath = vaultBase ? displayPath(vaultBase, home) : null;

  return (
    <div className="flex flex-col gap-8">
      <section className="flex flex-col gap-3">
        <SectionLabel>
          <Trans>On this device</Trans>
        </SectionLabel>
        <div className="flex flex-col">
          <SettingsRow
            title={<Trans>Local profile</Trans>}
            description={<Trans>No Notare account required</Trans>}
            trailing={
              <span className="text-muted-foreground text-xs font-medium">
                <Trans>Personal workspace</Trans>
              </span>
            }
          />
          <SettingsRow
            title={<Trans>Database &amp; storage</Trans>}
            description={
              formattedVaultPath ?? (
                <Trans>Local SQLite database on this device</Trans>
              )
            }
            trailing={
              <Button
                variant="outline"
                size="sm"
                onClick={handleRevealVault}
                disabled={!vaultBase}
                className="h-8 text-xs"
              >
                <FolderIcon className="mr-1.5 size-3.5" />
                <Trans>Reveal in folder</Trans>
              </Button>
            }
          />
          <SettingsRow
            title={<Trans>Export notes</Trans>}
            description={
              <Trans>
                Markdown or JSON · All meetings, summaries, and action items
              </Trans>
            }
            trailing={
              <Button
                variant="outline"
                size="sm"
                onClick={handleExportNotes}
                disabled={!vaultBase}
                className="h-8 text-xs"
              >
                <DownloadIcon className="mr-1.5 size-3.5" />
                <Trans>Export</Trans>
              </Button>
            }
          />
        </div>
      </section>

      <section className="flex flex-col gap-3">
        <SectionLabel>
          <Trans>Notes storage &amp; audio retention</Trans>
        </SectionLabel>
        <StorageSettingsView />
      </section>

      <section className="flex flex-col gap-3">
        <SectionLabel>
          <Trans>Data controls</Trans>
        </SectionLabel>
        <div className="flex flex-col">
          <SettingsRow
            title={<Trans>Delete all local data</Trans>}
            description={
              <Trans>
                Removes notes, recordings, and saved credentials from this
                device
              </Trans>
            }
            trailing={
              <Button
                variant="outline"
                size="sm"
                onClick={() => setResetModalOpen(true)}
                className="border-destructive/30 text-destructive hover:bg-destructive/10 hover:text-destructive h-8 text-xs"
              >
                <Trash2Icon className="mr-1.5 size-3.5" />
                <Trans>Review reset</Trans>
              </Button>
            }
          />
        </div>
      </section>

      <Dialog open={resetModalOpen} onOpenChange={setResetModalOpen}>
        <DialogContent className="max-w-md">
          <DialogHeader>
            <DialogTitle className="flex items-center gap-2 text-base">
              <TriangleAlertIcon className="text-destructive size-4" />
              <Trans>Delete all local data?</Trans>
            </DialogTitle>
            <DialogDescription className="text-xs">
              <Trans>
                This action cannot be undone. All notes, transcript recordings,
                and locally configured API keys will be deleted from this
                device.
              </Trans>
            </DialogDescription>
          </DialogHeader>
          <DialogFooter className="mt-4 gap-2">
            <Button
              variant="outline"
              size="sm"
              onClick={() => setResetModalOpen(false)}
            >
              <Trans>Cancel</Trans>
            </Button>
            <Button
              variant="destructive"
              size="sm"
              onClick={() => void handleConfirmReset()}
            >
              <Trans>Confirm reset</Trans>
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}

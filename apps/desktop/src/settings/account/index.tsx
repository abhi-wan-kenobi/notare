import { Trans, useLingui } from "@lingui/react/macro";
import { useQuery } from "@tanstack/react-query";
import { homeDir } from "@tauri-apps/api/path";
import { FolderIcon, HelpCircleIcon } from "lucide-react";
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
  const { data: dataBase, isError: dataPathError } = useQuery({
    queryKey: ["global-base-path"],
    queryFn: async () => {
      const result = await settingsCommands.globalBase();
      if (result.status === "error") {
        throw new Error(result.error);
      }
      return result.data;
    },
  });
  const { data: vaultBase, isError: vaultPathError } = useQuery({
    queryKey: ["vault-base-path"],
    queryFn: async () => {
      const result = await settingsCommands.vaultBase();
      if (result.status === "error") {
        throw new Error(result.error);
      }
      return result.data;
    },
  });

  const handleRevealData = () => {
    if (dataBase) {
      void openerCommands.openPath(dataBase, null);
    }
  };

  const handleRevealVault = () => {
    if (vaultBase) {
      void openerCommands.openPath(vaultBase, null);
    }
  };

  const formattedDataPath = dataBase ? displayPath(dataBase, home) : null;

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
            title={<Trans>Application data</Trans>}
            description={
              formattedDataPath ?? (
                <Trans>Local application data on this device</Trans>
              )
            }
            trailing={
              <Button
                variant="outline"
                size="sm"
                onClick={handleRevealData}
                disabled={!dataBase}
                className="h-8 text-xs"
              >
                <FolderIcon className="mr-1.5 size-3.5" />
                <Trans>Reveal in folder</Trans>
              </Button>
            }
          />
          <SettingsRow
            title={<Trans>Notes folder</Trans>}
            description={
              <Trans>
                Browse locally stored notes and recordings. Use Export in an
                individual meeting for a document export.
              </Trans>
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
                <Trans>Open notes folder</Trans>
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
            title={<Trans>Manual reset help</Trans>}
            description={
              <Trans>
                Learn which local files to back up before a manual reset. No
                files are deleted by this action.
              </Trans>
            }
            trailing={
              <Button
                variant="outline"
                size="sm"
                onClick={() => setResetModalOpen(true)}
                className="h-8 text-xs"
              >
                <HelpCircleIcon className="mr-1.5 size-3.5" />
                <Trans>Reset instructions</Trans>
              </Button>
            }
          />
        </div>
      </section>

      <Dialog open={resetModalOpen} onOpenChange={setResetModalOpen}>
        <DialogContent className="max-w-md">
          <DialogHeader>
            <DialogTitle className="flex items-center gap-2 text-base">
              <HelpCircleIcon className="size-4" />
              <Trans>Reset instructions</Trans>
            </DialogTitle>
            <DialogDescription className="text-xs">
              <Trans>
                This is help only. Notare will not delete any files.
              </Trans>
            </DialogDescription>
          </DialogHeader>
          <ol className="text-muted-foreground list-decimal space-y-3 pl-5 text-xs">
            <li>
              <Trans>
                Back up your data and notes folders, then quit Notare before
                changing files.
              </Trans>
            </li>
            <li>
              <Trans>
                Removing database files (app.db and its companion files) deletes
                the meetings, notes, transcripts, summaries, action items, and
                preferences stored in that database. Markdown files are not a
                full database backup.
              </Trans>
            </li>
            <li>
              <Trans>
                Removing settings.json only removes legacy settings stored in
                that file, not current database preferences. Remove saved API
                keys in provider settings if needed; deleting database files
                does not revoke credentials or clear system keychain entries.
              </Trans>
            </li>
            <li>
              <Trans>
                Recordings, attachments, and Markdown files remain in the notes
                folder unless you remove them yourself. Do not delete an entire
                shared notes folder or unrelated user files.
              </Trans>
            </li>
          </ol>
          <dl className="space-y-3 text-xs">
            <div>
              <dt className="font-medium">
                <Trans>Application data directory</Trans>
              </dt>
              <dd className="text-muted-foreground mt-1 break-all select-text">
                {dataBase ??
                  (dataPathError
                    ? t`Could not load the data directory.`
                    : t`Loading data directory…`)}
              </dd>
            </div>
            <div>
              <dt className="font-medium">
                <Trans>Notes folder</Trans>
              </dt>
              <dd className="text-muted-foreground mt-1 break-all select-text">
                {vaultBase ??
                  (vaultPathError
                    ? t`Could not load the notes folder.`
                    : t`Loading notes folder…`)}
              </dd>
            </div>
          </dl>
          <DialogFooter className="mt-4 gap-2">
            <Button
              variant="outline"
              size="sm"
              onClick={() => setResetModalOpen(false)}
            >
              <Trans>Done</Trans>
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}

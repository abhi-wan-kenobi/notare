import { commands as calendarCommands } from "@hypr/plugin-calendar";
import type { IcsImportedFile } from "@hypr/plugin-calendar";
import { Trans, useLingui } from "@lingui/react/macro";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { open as selectFile } from "@tauri-apps/plugin-dialog";
import { RefreshCwIcon, Trash2Icon } from "lucide-react";
import { useCallback, useState } from "react";

import { useSync } from "../context";
import {
  OAuthCalendarSelection,
  useOAuthCalendarSelection,
} from "../oauth/calendar-selection";
import type { CalendarProvider } from "../shared";

const FILES_QUERY_KEY = ["ics-imported-files"];

function useImportedIcsFiles() {
  return useQuery({
    queryKey: FILES_QUERY_KEY,
    queryFn: async (): Promise<IcsImportedFile[]> => {
      const result = await calendarCommands.icsListFiles();
      if (result.status === "error") throw new Error(result.error);
      return result.data;
    },
  });
}

async function pickIcsFiles(multiple: boolean): Promise<string[] | null> {
  const picked = await selectFile({
    multiple,
    directory: false,
    filters: [{ name: "iCalendar file", extensions: ["ics"] }],
  });
  if (!picked) return null;
  return Array.isArray(picked) ? picked : [picked];
}

/**
 * Imported `.ics` calendar-file UI: import one or more files (each becomes a
 * calendar), list them with update/remove actions, and toggle the resulting
 * calendars like any other provider. Files are copied into the app data dir,
 * so the source can vanish.
 */
export function IcsContent({ config }: { config: CalendarProvider }) {
  const queryClient = useQueryClient();
  const { data: files, isPending, isError } = useImportedIcsFiles();
  const { scheduleSync } = useSync();
  const [actionError, setActionError] = useState<string | null>(null);

  const onChanged = useCallback(() => {
    setActionError(null);
    void queryClient.invalidateQueries({ queryKey: FILES_QUERY_KEY });
    scheduleSync();
  }, [queryClient, scheduleSync]);

  const importMutation = useMutation({
    mutationFn: async () => {
      const paths = await pickIcsFiles(true);
      if (!paths) return null;
      const result = await calendarCommands.icsImportFiles(paths);
      if (result.status === "error") throw new Error(result.error);
      return result.data;
    },
    onSuccess: (data) => {
      if (data) onChanged();
    },
    onError: (error: Error) => setActionError(error.message),
  });

  const replaceMutation = useMutation({
    mutationFn: async (id: string) => {
      const paths = await pickIcsFiles(false);
      if (!paths || paths.length === 0) return null;
      const result = await calendarCommands.icsReplaceFile(id, paths[0]);
      if (result.status === "error") throw new Error(result.error);
      return result.data;
    },
    onSuccess: (data) => {
      if (data) onChanged();
    },
    onError: (error: Error) => setActionError(error.message),
  });

  const removeMutation = useMutation({
    mutationFn: async (id: string) => {
      const result = await calendarCommands.icsRemoveFile(id);
      if (result.status === "error") throw new Error(result.error);
      return true;
    },
    onSuccess: onChanged,
    onError: (error: Error) => setActionError(error.message),
  });

  if (isPending) {
    return (
      <div className="pt-1 pb-2">
        <span className="text-muted-foreground text-xs">
          <Trans>Loading…</Trans>
        </span>
      </div>
    );
  }

  if (isError || !files) {
    return (
      <div className="pt-1 pb-2">
        <span className="text-xs text-red-600">
          <Trans>Failed to load imported calendar files</Trans>
        </span>
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-2 pt-1 pb-2">
      <p className="text-muted-foreground text-xs">
        <Trans>
          Import a calendar exported as an .ics file. The file is copied into
          Notare, so the original can be moved or deleted.
        </Trans>
      </p>
      <button
        onClick={() => importMutation.mutate()}
        disabled={importMutation.isPending}
        className="text-muted-foreground hover:text-foreground cursor-pointer self-start text-xs underline transition-colors disabled:cursor-default"
      >
        <Trans>Import calendar file (.ics)…</Trans>
      </button>

      {files.length > 0 && (
        <ul className="flex flex-col gap-1">
          {files.map((file) => (
            <IcsFileRow
              key={file.id}
              file={file}
              onReplace={() => replaceMutation.mutate(file.id)}
              onRemove={() => removeMutation.mutate(file.id)}
              isBusy={replaceMutation.isPending || removeMutation.isPending}
            />
          ))}
        </ul>
      )}

      {files.length > 0 && <IcsCalendarToggles config={config} />}

      {actionError && <p className="text-xs text-red-600">{actionError}</p>}
    </div>
  );
}

function IcsFileRow({
  file,
  onReplace,
  onRemove,
  isBusy,
}: {
  file: IcsImportedFile;
  onReplace: () => void;
  onRemove: () => void;
  isBusy: boolean;
}) {
  const { t } = useLingui();

  return (
    <li className="group flex items-center gap-2">
      <div className="flex min-w-0 flex-1 flex-col">
        <span className="text-foreground truncate text-xs font-medium">
          {file.title}
        </span>
        <span className="text-muted-foreground truncate text-[11px]">
          {file.file_name} · {file.event_count}{" "}
          {file.event_count === 1 ? t`event` : t`events`}
        </span>
      </div>
      <button
        onClick={onReplace}
        disabled={isBusy}
        title={t`Update from a new file`}
        aria-label={t`Update ${file.title} from a new file`}
        className="text-muted-foreground hover:text-foreground shrink-0 cursor-pointer rounded p-1 transition-colors disabled:cursor-default"
      >
        <RefreshCwIcon className="size-3" />
      </button>
      <button
        onClick={onRemove}
        disabled={isBusy}
        title={t`Remove`}
        aria-label={t`Remove ${file.title}`}
        className="text-muted-foreground shrink-0 cursor-pointer rounded p-1 transition-colors hover:text-red-600 disabled:cursor-default"
      >
        <Trash2Icon className="size-3" />
      </button>
    </li>
  );
}

/** Enable/disable toggles for the calendars synced from the imported files. */
function IcsCalendarToggles({ config }: { config: CalendarProvider }) {
  const { groups, handleRefresh, handleToggle, isLoading } =
    useOAuthCalendarSelection(config);

  return (
    <OAuthCalendarSelection
      groups={groups}
      onToggle={handleToggle}
      onRefresh={handleRefresh}
      isLoading={isLoading}
    />
  );
}

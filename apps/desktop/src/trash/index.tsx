import { Trans, useLingui } from "@lingui/react/macro";
import { Trash2Icon, Undo2Icon } from "lucide-react";
import { useState } from "react";

import { Button } from "@hypr/ui/components/ui/button";
import { Spinner } from "@hypr/ui/components/ui/spinner";
import { cn, format, safeParseDate } from "@hypr/utils";

import { TrashConfirmDialog } from "./confirm-dialog";
import {
  useEmptyTrash,
  useHardDeleteTrashedSession,
  useRestoreTrashedSession,
} from "./queries";

import type { TrashedSessionRecord } from "~/session/queries";
import { useTrashedSessions } from "~/session/queries";
import { StandardContentWrapper } from "~/shared/main";

/**
 * Trash: every session carrying a `deleted_at` tombstone, newest deletion
 * first. Restore reuses the exact tombstone-cleared path the undo toast uses
 * (no confirm - deleting elsewhere already offered its 5s undo); "Delete
 * forever" hard-DELETEs DB rows plus the session folder, behind a confirm.
 */
export function TabContentTrash() {
  const { t } = useLingui();
  const sessions = useTrashedSessions();

  const restore = useRestoreTrashedSession();
  const deleteForever = useHardDeleteTrashedSession();
  const emptyTrash = useEmptyTrash();

  const [pendingDelete, setPendingDelete] =
    useState<TrashedSessionRecord | null>(null);
  const [emptyConfirmOpen, setEmptyConfirmOpen] = useState(false);

  const isEmpty = sessions.length === 0;
  const deleteError = deleteForever.error ?? emptyTrash.error;

  return (
    <StandardContentWrapper>
      <div className="flex h-full flex-col">
        <div className="border-border/60 flex shrink-0 items-center justify-between gap-3 border-b p-4">
          <div className="flex min-w-0 items-center gap-2">
            <Trash2Icon className="text-muted-foreground h-4 w-4 shrink-0" />
            <h1 className="text-foreground truncate text-sm font-semibold">
              <Trans>Trash</Trans>
            </h1>
            {!isEmpty && (
              <span className="text-muted-foreground text-xs">
                {t`${sessions.length} item(s)`}
              </span>
            )}
          </div>
          {!isEmpty && (
            <Button
              variant="outline"
              size="sm"
              disabled={emptyTrash.isPending || deleteForever.isPending}
              onClick={() => setEmptyConfirmOpen(true)}
            >
              {emptyTrash.isPending ? (
                <Spinner className="h-3.5 w-3.5" />
              ) : (
                <Trash2Icon className="size-3.5" />
              )}
              <Trans>Empty trash</Trans>
            </Button>
          )}
        </div>

        <div className="min-h-0 flex-1 overflow-y-auto p-4">
          {isEmpty ? (
            <EmptyTrashState />
          ) : (
            <ul
              className="mx-auto flex max-w-2xl flex-col gap-2"
              data-testid="trash-session-list"
            >
              {sessions.map((session) => (
                <TrashSessionRow
                  key={session.id}
                  session={session}
                  isPending={
                    (restore.isPending && restore.variables === session.id) ||
                    (deleteForever.isPending &&
                      deleteForever.variables === session.id)
                  }
                  onRestore={() => restore.mutate(session.id)}
                  onDeleteForever={() => setPendingDelete(session)}
                />
              ))}
            </ul>
          )}
        </div>
      </div>

      <TrashConfirmDialog
        open={pendingDelete !== null}
        onOpenChange={(open) => {
          if (!open) setPendingDelete(null);
        }}
        title={t`Delete this note forever?`}
        description={
          pendingDelete
            ? t`"${pendingDelete.title || t`Untitled`}" will be permanently deleted. This cannot be undone.`
            : ""
        }
        confirmLabel={t`Delete forever`}
        isPending={deleteForever.isPending}
        error={deleteError instanceof Error ? deleteError.message : null}
        onConfirm={() => {
          const session = pendingDelete;
          setPendingDelete(null);
          if (session) {
            deleteForever.mutate(session.id);
          }
        }}
      />

      <TrashConfirmDialog
        open={emptyConfirmOpen}
        onOpenChange={setEmptyConfirmOpen}
        title={t`Empty trash?`}
        description={t`All ${sessions.length} trashed items will be permanently deleted. This cannot be undone.`}
        confirmLabel={t`Empty trash`}
        isPending={emptyTrash.isPending}
        error={deleteError instanceof Error ? deleteError.message : null}
        onConfirm={() => {
          setEmptyConfirmOpen(false);
          emptyTrash.mutate();
        }}
      />
    </StandardContentWrapper>
  );
}

function TrashSessionRow({
  session,
  isPending,
  onRestore,
  onDeleteForever,
}: {
  session: TrashedSessionRecord;
  isPending: boolean;
  onRestore: () => void;
  onDeleteForever: () => void;
}) {
  const { t } = useLingui();
  const deletedDate = safeParseDate(session.deleted_at);

  return (
    <li
      data-testid="trash-session-row"
      data-session-id={session.id}
      className={cn([
        "border-border group flex items-center gap-3 rounded-lg border p-3",
        "hover:bg-accent/20",
        isPending && "opacity-60",
      ])}
    >
      <div className="flex min-w-0 flex-1 flex-col gap-0.5">
        <p className="truncate text-sm font-normal">
          {session.title || t`Untitled`}
        </p>
        <div className="text-muted-foreground flex items-center gap-1.5 font-mono text-xs">
          {deletedDate && (
            <span title={t`Deleted ${format(deletedDate, "PPP p")}`}>
              {format(deletedDate, "PPP")}
            </span>
          )}
          {session.preview && (
            <>
              <span aria-hidden="true">·</span>
              <span className="truncate">{session.preview}</span>
            </>
          )}
        </div>
      </div>
      <div className="flex shrink-0 items-center gap-1">
        <Button
          variant="ghost"
          size="sm"
          disabled={isPending}
          onClick={onRestore}
        >
          <Undo2Icon className="size-3.5" />
          <Trans>Restore</Trans>
        </Button>
        <Button
          variant="ghost"
          size="sm"
          className="text-red-600 hover:text-red-700 dark:text-red-400 dark:hover:text-red-300"
          disabled={isPending}
          onClick={onDeleteForever}
        >
          <Trash2Icon className="size-3.5" />
          <Trans>Delete forever</Trans>
        </Button>
      </div>
    </li>
  );
}

function EmptyTrashState() {
  const { t } = useLingui();
  return (
    <div className="text-muted-foreground flex h-full flex-col items-center justify-center gap-2 px-6 text-center">
      <div className="text-muted-foreground/70">
        <Trash2Icon className="h-6 w-6" />
      </div>
      <p className="text-foreground text-sm font-medium">{t`Trash is empty`}</p>
      <p className="max-w-sm text-xs">
        {t`Notes you delete land here first and stay until you empty the trash.`}
      </p>
    </div>
  );
}

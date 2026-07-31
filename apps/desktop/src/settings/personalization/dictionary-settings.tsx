import { Trans, useLingui } from "@lingui/react/macro";
import { downloadDir, join } from "@tauri-apps/api/path";
import { open as selectFile } from "@tauri-apps/plugin-dialog";
import {
  ArrowRightIcon,
  CheckIcon,
  CircleMinusIcon,
  DownloadIcon,
  PencilIcon,
  PlusIcon,
  UploadIcon,
  XIcon,
} from "lucide-react";
import { useMemo, useState } from "react";

import { commands as fs2Commands } from "@hypr/plugin-fs2";
import { commands as openerCommands } from "@hypr/plugin-opener2";
import { Button } from "@hypr/ui/components/ui/button";
import { Input } from "@hypr/ui/components/ui/input";
import { Switch } from "@hypr/ui/components/ui/switch";
import { cn } from "@hypr/utils";

import {
  type DictionaryEntry,
  type DictionaryMapping,
  exportDictionaryText,
  importDictionaryText,
  parseDictionaryEntries,
  serializeDictionaryEntries,
} from "~/dictation/dictionary";
import { parseDictionaryTermsText } from "~/stt/keywords";

// The wrong->right correction mappings and legacy flat hint terms are both
// stored as one JSON array in `personalization_dictionary_terms` (a bare
// string element = flat term, an object = a mapping). We work off the raw
// stored string directly (not `useConfigValue`, whose JSON_PARSED_KEYS path
// flattens this setting down to `string[]` and silently drops mapping
// objects) so mappings round-trip losslessly.
export function DictionarySettings({
  raw,
  onSave,
}: {
  raw: string;
  onSave: (value: string) => void;
}) {
  const { t } = useLingui();

  const entries = useMemo(() => parseDictionaryEntries(raw), [raw]);
  const existingKeys = useMemo(() => new Set(entries.map(entryKey)), [entries]);

  const persist = (nextEntries: DictionaryEntry[]) => {
    onSave(serializeDictionaryEntries(nextEntries));
  };

  // --- add form -------------------------------------------------------
  const [addWrong, setAddWrong] = useState("");
  const [addRight, setAddRight] = useState("");
  const [addCaseSensitive, setAddCaseSensitive] = useState(false);

  const candidateEntries = useMemo<DictionaryEntry[]>(() => {
    // Whitespace-collapsed like the flat-term path (normalizeKeywordList) -
    // both entry kinds must normalize identically or space-variant
    // duplicates slip through.
    const right = collapseSpaces(addRight);
    if (right) {
      const wrong = collapseSpaces(addWrong);
      if (!wrong) return [];
      const mapping: DictionaryMapping = {
        wrong,
        right,
        caseSensitive: addCaseSensitive,
      };
      return [mapping];
    }
    // No replacement entered: fall back to the legacy flat-term behavior,
    // which lets you paste a comma/newline separated batch at once.
    return parseDictionaryTermsText(addWrong);
  }, [addWrong, addRight, addCaseSensitive]);

  const newCandidateEntries = candidateEntries.filter(
    (entry) => !existingKeys.has(entryKey(entry)),
  );
  const duplicateCandidateCount =
    candidateEntries.length - newCandidateEntries.length;
  const canAdd = newCandidateEntries.length > 0;

  const handleAdd = () => {
    if (!canAdd) return;
    persist([...entries, ...newCandidateEntries]);
    setAddWrong("");
    setAddRight("");
    setAddCaseSensitive(false);
  };

  // --- edit in place ----------------------------------------------------
  const [editingIndex, setEditingIndex] = useState<number | null>(null);
  const [editWrong, setEditWrong] = useState("");
  const [editRight, setEditRight] = useState("");
  const [editCaseSensitive, setEditCaseSensitive] = useState(false);

  const startEdit = (index: number, entry: DictionaryEntry) => {
    setEditingIndex(index);
    setEditWrong(typeof entry === "string" ? entry : entry.wrong);
    setEditRight(typeof entry === "string" ? "" : entry.right);
    setEditCaseSensitive(
      typeof entry === "string" ? false : entry.caseSensitive,
    );
  };

  const cancelEdit = () => setEditingIndex(null);

  const editCandidate: DictionaryEntry | null = useMemo(() => {
    if (editingIndex === null) return null;
    const wrong = collapseSpaces(editWrong);
    if (!wrong) return null;
    const right = collapseSpaces(editRight);
    return right
      ? ({
          wrong,
          right,
          caseSensitive: editCaseSensitive,
        } as DictionaryMapping)
      : wrong;
  }, [editingIndex, editWrong, editRight, editCaseSensitive]);

  const editDuplicateIndex =
    editCandidate === null
      ? -1
      : entries.findIndex(
          (entry, index) =>
            index !== editingIndex &&
            entryKey(entry) === entryKey(editCandidate),
        );
  const canSaveEdit = editCandidate !== null && editDuplicateIndex === -1;

  const saveEdit = () => {
    if (!canSaveEdit || editingIndex === null || editCandidate === null) return;
    persist(
      entries.map((entry, index) =>
        index === editingIndex ? editCandidate : entry,
      ),
    );
    setEditingIndex(null);
  };

  // --- remove -------------------------------------------------------
  const handleRemove = (index: number) => {
    persist(entries.filter((_, i) => i !== index));
    if (editingIndex === index) {
      setEditingIndex(null);
    } else if (editingIndex !== null && editingIndex > index) {
      // The array shifted under the open editor - keep it on the same entry.
      setEditingIndex(editingIndex - 1);
    }
  };

  // --- import / export --------------------------------------------------
  const [importMessage, setImportMessage] = useState<string | null>(null);
  const [importError, setImportError] = useState<string | null>(null);
  const [isImporting, setIsImporting] = useState(false);
  const [exportError, setExportError] = useState<string | null>(null);
  const [isExporting, setIsExporting] = useState(false);

  const handleImport = async () => {
    setImportMessage(null);
    setImportError(null);
    setIsImporting(true);
    try {
      const picked = await selectFile({
        multiple: false,
        directory: false,
        filters: [{ name: t`Text file`, extensions: ["txt"] }],
      });
      if (!picked || Array.isArray(picked)) {
        return;
      }

      const readResult = await fs2Commands.readTextFile(picked);
      if (readResult.status === "error") {
        setImportError(readResult.error);
        return;
      }

      const imported = importDictionaryText(readResult.data);
      const { merged, addedCount, updatedCount } = mergeDictionaryEntries(
        entries,
        imported,
      );

      if (addedCount === 0 && updatedCount === 0) {
        setImportMessage(
          t`Nothing new to import — every entry already matched.`,
        );
        return;
      }

      persist(merged);
      // Count-agnostic phrasing: the app has no plural-rule machinery yet.
      setImportMessage(
        t`Import finished: ${addedCount} added, ${updatedCount} updated.`,
      );
    } catch (error) {
      setImportError(error instanceof Error ? error.message : String(error));
    } finally {
      setIsImporting(false);
    }
  };

  const handleExport = async () => {
    setExportError(null);
    setIsExporting(true);
    try {
      const text = exportDictionaryText(entries);
      const dir = await downloadDir();
      const filename = `notare-dictionary_${new Date().toISOString().replace(/[:.]/g, "-")}.txt`;
      const path = await join(dir, filename);
      const result = await fs2Commands.writeTextFile(path, text);
      if (result.status === "error") {
        setExportError(result.error);
        return;
      }
      await openerCommands.revealItemInDir(path);
    } catch (error) {
      setExportError(error instanceof Error ? error.message : String(error));
    } finally {
      setIsExporting(false);
    }
  };

  return (
    <div className="flex flex-col gap-4">
      <div className="flex items-center justify-between gap-4">
        <h2 className="font-sans text-lg font-semibold">
          <Trans>Dictionary</Trans>
        </h2>
        <div className="flex items-center gap-2">
          <Button
            type="button"
            variant="ghost"
            size="sm"
            disabled={isImporting}
            onClick={() => void handleImport()}
          >
            <UploadIcon className="size-3.5" />
            <Trans>Import…</Trans>
          </Button>
          <Button
            type="button"
            variant="ghost"
            size="sm"
            disabled={isExporting || entries.length === 0}
            onClick={() => void handleExport()}
          >
            <DownloadIcon className="size-3.5" />
            <Trans>Export</Trans>
          </Button>
        </div>
      </div>

      <p className="text-muted-foreground text-sm">
        <Trans>
          Mappings fix recurring misrecognitions in your dictated text — "far
          eye" always becomes "FarEye". Flat terms just bias the recognizer
          toward names or jargon while you speak, without rewriting anything.
        </Trans>
      </p>

      {(importMessage || importError || exportError) && (
        <div className="flex flex-col gap-1">
          {importMessage && (
            <p className="text-muted-foreground text-xs">{importMessage}</p>
          )}
          {importError && (
            <p className="text-xs text-red-600">
              <Trans>Import failed: {importError}</Trans>
            </p>
          )}
          {exportError && (
            <p className="text-xs text-red-600">
              <Trans>Export failed: {exportError}</Trans>
            </p>
          )}
        </div>
      )}

      <form
        className="flex flex-col gap-2"
        onSubmit={(event) => {
          event.preventDefault();
          event.stopPropagation();
          handleAdd();
        }}
      >
        <div className="flex items-center gap-2">
          <Input
            className="flex-1"
            placeholder={t`Wrong text, or names/jargon to prefer`}
            value={addWrong}
            onChange={(event) => setAddWrong(event.target.value)}
            aria-label={t`Wrong text, or names/jargon to prefer`}
          />
          <ArrowRightIcon className="text-muted-foreground size-4 shrink-0" />
          <Input
            className="flex-1"
            placeholder={t`Replace with (optional)`}
            value={addRight}
            onChange={(event) => {
              setAddRight(event.target.value);
              // Clearing the replacement hides the case toggle; a stale flag
              // must not silently ride along when one is typed again later.
              if (!event.target.value.trim()) {
                setAddCaseSensitive(false);
              }
            }}
            aria-label={t`Replace with (optional)`}
          />
        </div>

        {addRight.trim().length > 0 && (
          <label className="text-muted-foreground flex w-fit items-center gap-2 text-xs">
            <Switch
              size="sm"
              checked={addCaseSensitive}
              onCheckedChange={setAddCaseSensitive}
            />
            <Trans>Case-sensitive</Trans>
          </label>
        )}

        <div className="flex items-center justify-between gap-2">
          {duplicateCandidateCount > 0 ? (
            <p className="text-muted-foreground text-xs">
              <Trans>
                {duplicateCandidateCount} already in your dictionary — skipped.
              </Trans>
            </p>
          ) : (
            <span />
          )}
          <Button
            type="submit"
            variant="outline"
            size="sm"
            disabled={!canAdd}
            className={cn([
              canAdd
                ? "bg-black text-white hover:bg-black/90 hover:text-white dark:bg-white dark:text-black dark:hover:bg-white/90 dark:hover:text-black"
                : null,
            ])}
          >
            <PlusIcon className="size-3.5" />
            <Trans>Add</Trans>
          </Button>
        </div>
      </form>

      {entries.length === 0 ? (
        <p className="text-muted-foreground px-1 text-sm">
          <Trans>No dictionary entries yet.</Trans>
        </p>
      ) : (
        <div className="border-border bg-card divide-border divide-y overflow-hidden rounded-2xl border">
          {entries.map((entry, index) =>
            editingIndex === index ? (
              <DictionaryEditRow
                key={index}
                wrong={editWrong}
                right={editRight}
                caseSensitive={editCaseSensitive}
                canSave={canSaveEdit}
                onWrongChange={setEditWrong}
                onRightChange={setEditRight}
                onCaseSensitiveChange={setEditCaseSensitive}
                onSave={saveEdit}
                onCancel={cancelEdit}
              />
            ) : (
              <DictionaryRow
                key={index}
                entry={entry}
                onEdit={() => startEdit(index, entry)}
                onRemove={() => handleRemove(index)}
              />
            ),
          )}
        </div>
      )}
    </div>
  );
}

function DictionaryRow({
  entry,
  onEdit,
  onRemove,
}: {
  entry: DictionaryEntry;
  onEdit: () => void;
  onRemove: () => void;
}) {
  const { t } = useLingui();
  const isMapping = typeof entry !== "string";
  const label = isMapping ? `${entry.wrong} → ${entry.right}` : entry;

  return (
    <div className="group flex min-h-12 items-center justify-between gap-3 py-3 pr-3 pl-4">
      <div className="flex min-w-0 flex-1 items-center gap-2 text-sm">
        {isMapping ? (
          <>
            <span className="truncate">{entry.wrong}</span>
            <ArrowRightIcon className="text-muted-foreground size-3.5 shrink-0" />
            <span className="truncate font-medium">{entry.right}</span>
            {entry.caseSensitive && (
              <span className="text-muted-foreground border-border shrink-0 rounded border px-1 text-[10px] font-medium">
                <Trans>Aa</Trans>
              </span>
            )}
          </>
        ) : (
          <span className="truncate">{entry}</span>
        )}
      </div>
      <div className="flex shrink-0 items-center gap-1 opacity-0 transition-opacity group-hover:opacity-100 focus-within:opacity-100">
        <Button
          type="button"
          variant="ghost"
          size="icon"
          className="text-muted-foreground hover:text-foreground size-7"
          onClick={onEdit}
          aria-label={t`Edit ${label}`}
        >
          <PencilIcon className="size-3.5" />
        </Button>
        <Button
          type="button"
          variant="ghost"
          size="icon"
          className="text-muted-foreground hover:text-foreground size-7"
          onClick={onRemove}
          aria-label={t`Remove ${label}`}
        >
          <CircleMinusIcon className="size-4" />
        </Button>
      </div>
    </div>
  );
}

function DictionaryEditRow({
  wrong,
  right,
  caseSensitive,
  canSave,
  onWrongChange,
  onRightChange,
  onCaseSensitiveChange,
  onSave,
  onCancel,
}: {
  wrong: string;
  right: string;
  caseSensitive: boolean;
  canSave: boolean;
  onWrongChange: (value: string) => void;
  onRightChange: (value: string) => void;
  onCaseSensitiveChange: (value: boolean) => void;
  onSave: () => void;
  onCancel: () => void;
}) {
  const { t } = useLingui();

  return (
    <div className="flex flex-col gap-2 py-3 pr-3 pl-4">
      <div className="flex items-center gap-2">
        <Input
          className="flex-1"
          value={wrong}
          onChange={(event) => onWrongChange(event.target.value)}
          aria-label={t`Edit wrong text`}
        />
        <ArrowRightIcon className="text-muted-foreground size-4 shrink-0" />
        <Input
          className="flex-1"
          value={right}
          placeholder={t`Replace with (optional)`}
          onChange={(event) => onRightChange(event.target.value)}
          aria-label={t`Edit replacement text`}
        />
        <Button
          type="button"
          variant="ghost"
          size="icon"
          className="text-muted-foreground hover:text-foreground size-7 shrink-0"
          disabled={!canSave}
          onClick={onSave}
          aria-label={t`Save`}
        >
          <CheckIcon className="size-4" />
        </Button>
        <Button
          type="button"
          variant="ghost"
          size="icon"
          className="text-muted-foreground hover:text-foreground size-7 shrink-0"
          onClick={onCancel}
          aria-label={t`Cancel`}
        >
          <XIcon className="size-4" />
        </Button>
      </div>
      {right.trim().length > 0 && (
        <label className="text-muted-foreground flex w-fit items-center gap-2 text-xs">
          <Switch
            size="sm"
            checked={caseSensitive}
            onCheckedChange={onCaseSensitiveChange}
          />
          <Trans>Case-sensitive</Trans>
        </label>
      )}
      {!canSave && (
        <p className="text-muted-foreground text-xs">
          <Trans>Already used by another entry, or the text is empty.</Trans>
        </p>
      )}
    </div>
  );
}

/** Collapse runs of whitespace: STT text never carries doubles, and a
 * space-variant duplicate ("Far  Eye" vs "Far Eye") is always an accident. */
function collapseSpaces(value: string): string {
  return value.trim().replace(/\s+/g, " ");
}

export function entryKey(entry: DictionaryEntry): string {
  const text = typeof entry === "string" ? entry : entry.wrong;
  // Whitespace-collapsed to match the chat correction tool's dictionaryKey -
  // the two surfaces must agree on what counts as a duplicate.
  return collapseSpaces(text).toLocaleLowerCase();
}

function entriesEqual(a: DictionaryEntry, b: DictionaryEntry): boolean {
  if (typeof a === "string" && typeof b === "string") {
    return a.trim().toLocaleLowerCase() === b.trim().toLocaleLowerCase();
  }
  if (typeof a === "string" || typeof b === "string") {
    return false;
  }
  return (
    a.wrong === b.wrong &&
    a.right === b.right &&
    a.caseSensitive === b.caseSensitive
  );
}

export function mergeDictionaryEntries(
  existing: DictionaryEntry[],
  incoming: DictionaryEntry[],
): {
  merged: DictionaryEntry[];
  addedCount: number;
  updatedCount: number;
} {
  const merged = [...existing];
  const indexByKey = new Map(
    merged.map((entry, index) => [entryKey(entry), index]),
  );
  let addedCount = 0;
  let updatedCount = 0;

  for (const entry of incoming) {
    const key = entryKey(entry);
    if (!key) continue;

    const existingIndex = indexByKey.get(key);
    if (existingIndex === undefined) {
      indexByKey.set(key, merged.length);
      merged.push(entry);
      addedCount++;
      continue;
    }

    // Incoming wins on conflict - EXCEPT a bare term must never downgrade an
    // existing wrong->right mapping to a flat hint: importing a plain term
    // list into a mapping-rich dictionary would silently strip every rewrite.
    const current = merged[existingIndex];
    if (typeof entry === "string" && typeof current !== "string") {
      continue;
    }
    if (!entriesEqual(current, entry)) {
      merged[existingIndex] = entry;
      updatedCount++;
    }
  }

  return { merged, addedCount, updatedCount };
}

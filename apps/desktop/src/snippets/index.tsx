import { Trans, useLingui } from "@lingui/react/macro";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { PinIcon, ScrollTextIcon, SearchIcon } from "lucide-react";
import { useEffect, useMemo, useState } from "react";

import { commands as dictationCommands } from "@hypr/plugin-dictation";
import { Button } from "@hypr/ui/components/ui/button";
import { Spinner } from "@hypr/ui/components/ui/spinner";
import { sonnerToast } from "@hypr/ui/components/ui/toast";
import { format } from "@hypr/utils";

import { suggestCorrections } from "./correction-suggest";
import { type DayBucket, groupEntriesByDay } from "./day-grouping";
import { SnippetEntryRow } from "./entry-row";
import {
  addSuggestedDictionaryMappings,
  useDeleteSnippet,
  useSetSnippetPinned,
  useSnippetsHistoryQuery,
  useUpdateSnippetText,
} from "./queries";

import type { DictationHistoryEntry } from "~/dictation/history";
import { showTransientToast } from "~/sidebar/toast/transient";
import { StandardContentWrapper } from "~/shared/main";

/**
 * Duration for the "add to dictionary?" suggestion toast - long enough to
 * read a wrong -> right pair and decide, unlike the 2.4s default transient
 * toast (a quick "Copied" confirmation).
 */
const SUGGESTION_TOAST_DURATION_MS = 8000;
const SUGGESTION_TOAST_ID = "snippet-dictionary-suggest";

const SEARCH_DEBOUNCE_MS = 250;

function useDebouncedValue<T>(value: T, delayMs: number): T {
  const [debounced, setDebounced] = useState(value);
  useEffect(() => {
    const timer = setTimeout(() => setDebounced(value), delayMs);
    return () => clearTimeout(timer);
  }, [value, delayMs]);
  return debounced;
}

/**
 * Snippets: the promoted, first-class view of dictation history (previously
 * buried in Settings > Dictation). Day-grouped, server-side searched over
 * both cleaned and raw text, keyset-paginated via "Load more".
 */
export function TabContentSnippets() {
  const { t } = useLingui();
  const [query, setQuery] = useState("");
  const debouncedQuery = useDebouncedValue(query, SEARCH_DEBOUNCE_MS);
  const trimmedQuery = debouncedQuery.trim();
  const hasQuery = trimmedQuery.length > 0;

  const {
    data,
    isLoading,
    isError,
    hasNextPage,
    isFetchingNextPage,
    fetchNextPage,
  } = useSnippetsHistoryQuery(debouncedQuery);

  const setPinned = useSetSnippetPinned();
  const deleteSnippet = useDeleteSnippet();
  const updateText = useUpdateSnippetText();

  const entries = useMemo(
    () => data?.pages.flatMap((page) => page.entries) ?? [],
    [data],
  );

  const { pinnedEntries, dayBuckets } = useMemo(() => {
    const pinned = entries.filter((entry) => entry.pinned);
    const rest = entries.filter((entry) => !entry.pinned);
    return { pinnedEntries: pinned, dayBuckets: groupEntriesByDay(rest) };
  }, [entries]);

  const handleCopy = async (entry: DictationHistoryEntry) => {
    try {
      await writeText(entry.text);
    } catch {
      // Fall back to the browser clipboard when the plugin is unavailable.
      await navigator.clipboard.writeText(entry.text);
    }
    sonnerToast.success(t`Copied to clipboard`);
  };

  const handleInsert = async (entry: DictationHistoryEntry) => {
    try {
      const result = await dictationCommands.deliverText(entry.text, true);
      if (result.status === "error") {
        sonnerToast.error(t`Couldn't insert text`);
      }
    } catch {
      sonnerToast.error(t`Couldn't insert text`);
    }
  };

  const handleTogglePinned = (entry: DictationHistoryEntry) => {
    setPinned.mutate(
      { id: entry.id, pinned: !entry.pinned },
      { onError: () => sonnerToast.error(t`Couldn't update pin`) },
    );
  };

  const handleDelete = (entry: DictationHistoryEntry) => {
    deleteSnippet.mutate(entry.id, {
      onError: () => sonnerToast.error(t`Couldn't delete snippet`),
    });
  };

  /**
   * Suggest, never auto-learn: after a successful text save, diff the
   * before/after and - only if the user then explicitly taps "Add" on the
   * resulting toast - append the candidate(s) as dictionary mappings.
   * Dismissing (or letting the toast time out) teaches nothing.
   */
  const offerDictionarySuggestion = (before: string, after: string) => {
    const candidates = suggestCorrections(before, after);
    if (candidates.length === 0) {
      return;
    }

    // Name EVERY mapping the Add button will create - the consent tap should
    // cover exactly what's written on the toast, not a "+N more" mystery box.
    const pairs = candidates
      .map((candidate) => `"${candidate.wrong}" → "${candidate.right}"`)
      .join(", ");
    const description = t`Add ${pairs} to your dictionary?`;

    showTransientToast(
      {
        id: SUGGESTION_TOAST_ID,
        description,
        dismissible: true,
        primaryAction: {
          label: t`Add`,
          onClick: async () => {
            try {
              const { added } = await addSuggestedDictionaryMappings(candidates);
              if (added.length > 0) {
                sonnerToast.success(
                  added.length === 1
                    ? t`Added to your dictionary`
                    : t`Added ${added.length} entries to your dictionary`,
                );
              }
            } catch (error) {
              // The Add tap was consumed - a silent failure would read as
              // success.
              console.error("[snippets] failed to add dictionary mappings", error);
              sonnerToast.error(t`Couldn't update the dictionary. Try again.`);
            }
          },
        },
      },
      { durationMs: SUGGESTION_TOAST_DURATION_MS },
    );
  };

  const handleEditSave = (entry: DictationHistoryEntry, newText: string) => {
    const before = entry.text;
    updateText.mutate(
      { id: entry.id, text: newText },
      {
        onSuccess: () => offerDictionarySuggestion(before, newText),
        onError: () => sonnerToast.error(t`Couldn't save snippet`),
      },
    );
  };

  const isEmpty = !isLoading && !isError && entries.length === 0;
  const showNoHistory = isEmpty && !hasQuery;
  const showNoResults = isEmpty && hasQuery;

  return (
    <StandardContentWrapper>
      <div className="flex h-full flex-col">
        <div className="border-border/60 shrink-0 border-b p-4">
          <div className="bg-background border-border/80 focus-within:ring-ring flex items-center gap-3 rounded-full border px-4 py-2 focus-within:ring-1">
            <SearchIcon className="text-muted-foreground h-4 w-4 shrink-0" />
            <input
              // eslint-disable-next-line jsx-a11y/no-autofocus
              autoFocus
              value={query}
              onChange={(event) => setQuery(event.target.value)}
              placeholder={t`Search your dictation history...`}
              aria-label={t`Search your dictation history`}
              className="placeholder:text-muted-foreground flex-1 bg-transparent text-sm outline-hidden"
            />
            {hasQuery && isLoading ? (
              <Spinner className="text-muted-foreground h-4 w-4 shrink-0" />
            ) : null}
          </div>
        </div>

        <div className="min-h-0 flex-1 overflow-y-auto p-4">
          {isLoading ? (
            <LoadingSkeleton />
          ) : isError ? (
            <EmptyState
              icon={<ScrollTextIcon className="h-6 w-6" />}
              title={t`Couldn't load history`}
              description={t`Something went wrong loading your dictation history. Try again in a moment.`}
            />
          ) : showNoHistory ? (
            <EmptyState
              icon={<ScrollTextIcon className="h-6 w-6" />}
              title={t`No snippets yet`}
              description={t`Finished dictations show up here so you can find, reuse and pin them later.`}
            />
          ) : showNoResults ? (
            <EmptyState
              icon={<SearchIcon className="h-6 w-6" />}
              title={t`No results`}
              description={t`Nothing matched "${trimmedQuery}". Try different words.`}
            />
          ) : (
            <div className="mx-auto flex max-w-2xl flex-col gap-6">
              {pinnedEntries.length > 0 ? (
                <section className="flex flex-col gap-2">
                  <SectionHeader
                    icon={<PinIcon className="h-3.5 w-3.5" />}
                    label={<Trans>Pinned</Trans>}
                  />
                  <ul
                    className="flex flex-col gap-2"
                    data-testid="snippets-pinned-list"
                  >
                    {pinnedEntries.map((entry) => (
                      <SnippetEntryRow
                        key={entry.id}
                        entry={entry}
                        onCopy={(e) => void handleCopy(e)}
                        onInsert={(e) => void handleInsert(e)}
                        onTogglePinned={handleTogglePinned}
                        onDelete={handleDelete}
                        onEditSave={handleEditSave}
                      />
                    ))}
                  </ul>
                </section>
              ) : null}

              {dayBuckets.map((bucket) => (
                <DayBucketSection
                  key={bucket.key}
                  bucket={bucket}
                  onCopy={(e) => void handleCopy(e)}
                  onInsert={(e) => void handleInsert(e)}
                  onTogglePinned={handleTogglePinned}
                  onDelete={handleDelete}
                  onEditSave={handleEditSave}
                />
              ))}

              {hasNextPage ? (
                <div className="flex justify-center pb-2">
                  <Button
                    variant="outline"
                    size="sm"
                    disabled={isFetchingNextPage}
                    onClick={() => void fetchNextPage()}
                  >
                    {isFetchingNextPage ? (
                      <Spinner className="h-3.5 w-3.5" />
                    ) : (
                      <Trans>Load more</Trans>
                    )}
                  </Button>
                </div>
              ) : null}
            </div>
          )}
        </div>
      </div>
    </StandardContentWrapper>
  );
}

function DayBucketSection({
  bucket,
  onCopy,
  onInsert,
  onTogglePinned,
  onDelete,
  onEditSave,
}: {
  bucket: DayBucket;
  onCopy: (entry: DictationHistoryEntry) => void;
  onInsert: (entry: DictationHistoryEntry) => void;
  onTogglePinned: (entry: DictationHistoryEntry) => void;
  onDelete: (entry: DictationHistoryEntry) => void;
  onEditSave: (entry: DictationHistoryEntry, newText: string) => void;
}) {
  return (
    <section className="flex flex-col gap-2">
      <SectionHeader label={<DayBucketLabel bucket={bucket.bucket} />} />
      <ul className="flex flex-col gap-2">
        {bucket.entries.map((entry) => (
          <SnippetEntryRow
            key={entry.id}
            entry={entry}
            onCopy={onCopy}
            onInsert={onInsert}
            onTogglePinned={onTogglePinned}
            onDelete={onDelete}
            onEditSave={onEditSave}
          />
        ))}
      </ul>
    </section>
  );
}

function DayBucketLabel({ bucket }: { bucket: DayBucket["bucket"] }) {
  if (bucket.kind === "today") {
    return <Trans>Today</Trans>;
  }
  if (bucket.kind === "yesterday") {
    return <Trans>Yesterday</Trans>;
  }
  if (bucket.kind === "unknown") {
    return <Trans>Unknown date</Trans>;
  }
  return <>{format(bucket.date, "PPP")}</>;
}

function SectionHeader({
  icon,
  label,
}: {
  icon?: React.ReactNode;
  label: React.ReactNode;
}) {
  return (
    <div className="text-foreground flex items-center gap-1.5 px-1 text-sm font-semibold">
      {icon}
      {label}
    </div>
  );
}

function EmptyState({
  icon,
  title,
  description,
}: {
  icon: React.ReactNode;
  title: string;
  description: string;
}) {
  return (
    <div className="text-muted-foreground flex h-full flex-col items-center justify-center gap-2 px-6 text-center">
      <div className="text-muted-foreground/70">{icon}</div>
      <p className="text-foreground text-sm font-medium">{title}</p>
      <p className="max-w-sm text-xs">{description}</p>
    </div>
  );
}

function LoadingSkeleton() {
  return (
    <div
      data-testid="snippets-loading-skeleton"
      className="mx-auto flex max-w-2xl flex-col gap-2"
    >
      {[0, 1, 2, 3, 4].map((index) => (
        <div
          key={index}
          className="border-border animate-pulse rounded-lg border p-3"
        >
          <div className="bg-accent h-4 w-3/4 rounded-xs" />
          <div className="bg-muted mt-2 h-3 w-1/3 rounded-xs" />
        </div>
      ))}
    </div>
  );
}

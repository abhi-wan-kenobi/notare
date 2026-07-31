import {
  useInfiniteQuery,
  useMutation,
  useQueryClient,
} from "@tanstack/react-query";

import type { CorrectionCandidate } from "./correction-suggest";

import {
  type DictionaryMapping,
  parseDictionaryEntries,
  serializeDictionaryEntries,
} from "~/dictation/dictionary";
import {
  deleteDictationHistoryEntry,
  DICTATION_HISTORY_PAGE_SIZE,
  listDictationHistory,
  setDictationHistoryPinned,
  updateDictationHistoryText,
} from "~/dictation/history";
import { updateSettingValue } from "~/settings/queries";

/**
 * Shared prefix for every Snippets list query. Mutations invalidate the
 * whole family (all search queries + pages) rather than trying to patch
 * cached pages in place - the list is small/local enough that a refetch is
 * cheap and this avoids subtle keyset-cursor bugs from optimistic splicing.
 */
const SNIPPETS_LIST_QUERY_KEY = ["snippets", "history"] as const;

/**
 * Day-grouped, searchable, cursor-paginated dictation history for the
 * Snippets page. `query` is expected to already be debounced by the caller.
 */
export function useSnippetsHistoryQuery(query: string) {
  const trimmedQuery = query.trim();

  return useInfiniteQuery({
    queryKey: [...SNIPPETS_LIST_QUERY_KEY, trimmedQuery] as const,
    queryFn: ({ pageParam }) =>
      listDictationHistory({
        query: trimmedQuery || undefined,
        cursor: pageParam,
        limit: DICTATION_HISTORY_PAGE_SIZE,
      }),
    initialPageParam: undefined as string | undefined,
    // `nextCursor` is `null` (not `undefined`) when there's no further page,
    // but react-query only treats `undefined` as "no next page" - a raw
    // `null` (or a degenerate empty-string cursor) pageParam would be treated
    // as a real cursor and re-fetch page one forever.
    getNextPageParam: (lastPage) => lastPage.nextCursor || undefined,
  });
}

export function useSetSnippetPinned() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: ({ id, pinned }: { id: string; pinned: boolean }) =>
      setDictationHistoryPinned(id, pinned),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: SNIPPETS_LIST_QUERY_KEY });
    },
    onError: (error) => {
      console.error("[useSetSnippetPinned]", error);
    },
  });
}

export function useDeleteSnippet() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (id: string) => deleteDictationHistoryEntry(id),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: SNIPPETS_LIST_QUERY_KEY });
    },
    onError: (error) => {
      console.error("[useDeleteSnippet]", error);
    },
  });
}

/** Save an inline edit to a snippet's cleaned text. */
export function useUpdateSnippetText() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: ({ id, text }: { id: string; text: string }) =>
      updateDictationHistoryText(id, text),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: SNIPPETS_LIST_QUERY_KEY });
    },
    onError: (error) => {
      console.error("[useUpdateSnippetText]", error);
    },
  });
}

/**
 * Whitespace-collapsed, case-insensitive dedupe key - mirrors `entryKey` in
 * `settings/personalization/dictionary-settings.tsx` (kept as a local
 * equivalent rather than importing that file, which is outside this lane's
 * ownership) so the two surfaces agree on what counts as a duplicate.
 */
function mappingKey(value: string): string {
  return value.trim().replace(/\s+/g, " ").toLocaleLowerCase();
}

/**
 * Append accepted correction-suggestion candidates to the dictionary as
 * `{ wrong, right, caseSensitive: false }` mappings, one-tap-confirm only:
 * this only ever runs from an explicit toast "Add" click, never
 * automatically. Skips any candidate whose wrong-key already exists among
 * the stored entries (flat terms count by their own text, mappings count by
 * both `wrong` and `right`) - mirrors the append pattern in
 * `chat/tools/session-correction.ts`'s `saveDictionaryTerms`.
 */
export async function addSuggestedDictionaryMappings(
  candidates: CorrectionCandidate[],
): Promise<{ added: DictionaryMapping[] }> {
  if (candidates.length === 0) {
    return { added: [] };
  }

  let added: DictionaryMapping[] = [];
  await updateSettingValue(
    "personalization_dictionary_terms",
    (storedValue) => {
      const entries = parseDictionaryEntries(
        typeof storedValue === "string" ? storedValue : "[]",
      );
      const existingKeys = new Set<string>();
      for (const entry of entries) {
        if (typeof entry === "string") {
          existingKeys.add(mappingKey(entry));
        } else {
          existingKeys.add(mappingKey(entry.wrong));
          existingKeys.add(mappingKey(entry.right));
        }
      }

      added = [];
      const newMappings: DictionaryMapping[] = [];
      for (const candidate of candidates) {
        const key = mappingKey(candidate.wrong);
        if (existingKeys.has(key)) {
          continue;
        }
        existingKeys.add(key);
        const mapping: DictionaryMapping = {
          wrong: candidate.wrong,
          right: candidate.right,
          caseSensitive: false,
        };
        newMappings.push(mapping);
        added.push(mapping);
      }

      return serializeDictionaryEntries([...entries, ...newMappings]);
    },
  );

  return { added };
}

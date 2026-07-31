import { useInfiniteQuery, useMutation, useQueryClient } from "@tanstack/react-query";

import {
  deleteDictationHistoryEntry,
  DICTATION_HISTORY_PAGE_SIZE,
  listDictationHistory,
  setDictationHistoryPinned,
} from "~/dictation/history";

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

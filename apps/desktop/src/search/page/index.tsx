import { Trans, useLingui } from "@lingui/react/macro";
import { FileTextIcon, MicIcon, SearchIcon } from "lucide-react";
import {
  Fragment,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";

import { commands as embeddingSearch } from "@hypr/plugin-embedding-search";
import { Card, CardContent, CardHeader } from "@hypr/ui/components/ui/card";
import { Spinner } from "@hypr/ui/components/ui/spinner";
import { cn } from "@hypr/utils";

import { splitHighlight } from "./highlight";

import { useSearchEngine } from "~/search/contexts/engine";
import {
  hybridBySession,
  type HybridResult,
  type LexicalHit,
  type SemanticHit,
} from "~/search/hybrid";
import { useSessionSummaries } from "~/session/queries";
import { StandardContentWrapper } from "~/shared/main";
import { type Tab, useTabs } from "~/store/zustand/tabs";

type SearchTab = Extract<Tab, { type: "search" }>;

/** How many chunk snippets to preview per grouped meeting card. */
const MAX_SNIPPETS_PER_CARD = 3;
/** Dense-arm fan-out; RRF re-ranks so a wide-ish k is fine. */
const SEMANTIC_K = 60;
const DEBOUNCE_MS = 250;

function useDebouncedValue<T>(value: T, delayMs: number): T {
  const [debounced, setDebounced] = useState(value);
  useEffect(() => {
    const timer = setTimeout(() => setDebounced(value), delayMs);
    return () => clearTimeout(timer);
  }, [value, delayMs]);
  return debounced;
}

export function TabContentSearch({ tab }: { tab: SearchTab }) {
  const { t } = useLingui();
  const { search: lexicalSearch, isIndexing } = useSearchEngine();
  const openNew = useTabs((state) => state.openNew);
  const sessions = useSessionSummaries();

  const sessionTitleById = useMemo(() => {
    const map = new Map<string, string>();
    for (const session of sessions) {
      map.set(session.id, session.title);
    }
    return map;
  }, [sessions]);

  const [query, setQuery] = useState(tab.query ?? "");
  const debouncedQuery = useDebouncedValue(query, DEBOUNCE_MS);

  const [results, setResults] = useState<HybridResult[]>([]);
  const [isSearching, setIsSearching] = useState(false);
  // Monotonic token so a slow response for an old query can't overwrite a newer one.
  const requestToken = useRef(0);

  useEffect(() => {
    const trimmed = debouncedQuery.trim();
    const token = ++requestToken.current;

    if (!trimmed) {
      setResults([]);
      setIsSearching(false);
      return;
    }

    setIsSearching(true);

    void Promise.all([
      // Lexical (Tantivy BM25) arm — reuses the shared search engine hook.
      lexicalSearch(trimmed, null)
        .then((hits): LexicalHit[] =>
          hits.map((hit) => ({
            id: hit.document.id,
            entityType: hit.document.type,
            title: hit.document.title,
            content: hit.document.content,
            score: hit.score,
          })),
        )
        .catch(() => [] as LexicalHit[]),
      // Dense (semantic) arm — swallow errors so the page still works with the
      // dense arm disabled (index not built / model not downloaded / S0 NO-GO).
      embeddingSearch
        .semanticSearch(trimmed, SEMANTIC_K, null)
        .then((result): SemanticHit[] =>
          result.status === "ok"
            ? result.data.map((hit) => ({
                chunkId: hit.chunkId,
                sessionId: hit.sessionId,
                sourceType: hit.sourceType,
                text: hit.text,
                startMs: hit.startMs,
                distance: hit.distance,
              }))
            : [],
        )
        .catch(() => [] as SemanticHit[]),
    ]).then(([lexical, semantic]) => {
      if (token !== requestToken.current) return;
      setResults(hybridBySession(lexical, semantic));
      setIsSearching(false);
    });
  }, [debouncedQuery, lexicalSearch]);

  const openSession = useCallback(
    (sessionId: string, _startMs?: number | null) => {
      // TODO(WS-B2): seek to `_startMs` after opening. The audio player's `seek`
      // lives inside the session-scoped AudioPlayerProvider
      // (apps/desktop/src/audio-player/provider.tsx); there is no store/event to
      // hand a pending seek target to a session that mounts fresh, so full
      // jump-to-timestamp is deferred. Transcript snippets already carry startMs,
      // so wiring is one hop once that channel exists.
      openNew({ type: "sessions", id: sessionId });
    },
    [openNew],
  );

  const trimmedQuery = debouncedQuery.trim();
  const hasQuery = trimmedQuery.length > 0;
  const showLoading = hasQuery && isSearching && results.length === 0;
  const showEmptyQuery = !hasQuery;
  const showNoResults = hasQuery && !isSearching && results.length === 0;

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
              placeholder={t`Search across all your meetings...`}
              className="placeholder:text-muted-foreground flex-1 bg-transparent text-sm outline-hidden"
            />
            {hasQuery && isSearching ? (
              <Spinner className="text-muted-foreground h-4 w-4 shrink-0" />
            ) : null}
          </div>
          {isIndexing ? (
            <p className="text-muted-foreground mt-2 px-1 text-xs">
              <Trans>Building the search index…</Trans>
            </p>
          ) : null}
        </div>

        <div className="min-h-0 flex-1 overflow-y-auto p-4">
          {showEmptyQuery ? (
            <EmptyState
              icon={<SearchIcon className="h-6 w-6" />}
              title={t`Search your meetings`}
              description={t`Find anything across your notes and transcripts — by keyword or by meaning.`}
            />
          ) : showLoading ? (
            <div className="flex justify-center py-12">
              <Spinner className="text-muted-foreground h-6 w-6" />
            </div>
          ) : showNoResults ? (
            <EmptyState
              icon={<SearchIcon className="h-6 w-6" />}
              title={t`No results`}
              description={t`Nothing matched "${trimmedQuery}". Try different words.`}
            />
          ) : (
            <div className="mx-auto flex max-w-2xl flex-col gap-3">
              {results.map((result) => (
                <SearchResultCard
                  key={result.sessionId}
                  result={result}
                  query={trimmedQuery}
                  title={
                    result.lexical?.title ??
                    sessionTitleById.get(result.sessionId) ??
                    t`Untitled`
                  }
                  onOpenSession={openSession}
                />
              ))}
            </div>
          )}
        </div>
      </div>
    </StandardContentWrapper>
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

type Snippet = {
  key: string;
  text: string;
  sourceType: string;
  startMs: number | null;
};

function SearchResultCard({
  result,
  query,
  title,
  onOpenSession,
}: {
  result: HybridResult;
  query: string;
  title: string;
  onOpenSession: (sessionId: string, startMs?: number | null) => void;
}) {
  const snippets: Snippet[] = useMemo(() => {
    if (result.semantic.length > 0) {
      return result.semantic.slice(0, MAX_SNIPPETS_PER_CARD).map((hit) => ({
        key: hit.chunkId,
        text: hit.text,
        sourceType: hit.sourceType,
        startMs: hit.startMs,
      }));
    }
    // Lexical-only match: fall back to the document content as one snippet.
    if (result.lexical?.content) {
      return [
        {
          key: `${result.sessionId}-lexical`,
          text: result.lexical.content,
          sourceType: "note",
          startMs: null,
        },
      ];
    }
    return [];
  }, [result]);

  return (
    <Card variant="default">
      <CardHeader spacing="compact">
        <button
          type="button"
          onClick={() => onOpenSession(result.sessionId)}
          className="hover:text-primary flex items-center gap-2 text-left text-sm font-semibold transition-colors"
        >
          <FileTextIcon className="text-muted-foreground h-4 w-4 shrink-0" />
          <span className="truncate">{title}</span>
        </button>
      </CardHeader>
      {snippets.length > 0 ? (
        <CardContent spacing="compact" className="flex flex-col gap-1.5">
          {snippets.map((snippet) => (
            <SnippetRow
              key={snippet.key}
              snippet={snippet}
              query={query}
              onClick={() => onOpenSession(result.sessionId, snippet.startMs)}
            />
          ))}
        </CardContent>
      ) : null}
    </Card>
  );
}

function SnippetRow({
  snippet,
  query,
  onClick,
}: {
  snippet: Snippet;
  query: string;
  onClick: () => void;
}) {
  const isTranscript = snippet.sourceType === "transcript";

  return (
    <button
      type="button"
      onClick={onClick}
      className={cn([
        "hover:bg-accent/60 group flex w-full items-start gap-2 rounded-md px-2 py-1.5 text-left transition-colors",
      ])}
    >
      {isTranscript ? (
        <MicIcon className="text-muted-foreground mt-0.5 h-3.5 w-3.5 shrink-0" />
      ) : (
        <FileTextIcon className="text-muted-foreground mt-0.5 h-3.5 w-3.5 shrink-0" />
      )}
      <span className="text-muted-foreground line-clamp-2 flex-1 text-xs leading-relaxed">
        <HighlightedText text={snippet.text} query={query} />
      </span>
    </button>
  );
}

function HighlightedText({ text, query }: { text: string; query: string }) {
  const parts = useMemo(() => splitHighlight(text, query), [text, query]);
  return (
    <>
      {parts.map((part, index) =>
        part.match ? (
          <mark
            key={index}
            className="text-foreground rounded-xs bg-yellow-200/70 dark:bg-yellow-700/50"
          >
            {part.text}
          </mark>
        ) : (
          <Fragment key={index}>{part.text}</Fragment>
        ),
      )}
    </>
  );
}

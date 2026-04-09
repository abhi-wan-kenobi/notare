import { useQuery } from "@tanstack/react-query";
import { fetch } from "@tauri-apps/plugin-http";
import { ExternalLinkIcon, PlusIcon, XIcon } from "lucide-react";
import { useEffect, useMemo, useState } from "react";

import { commands as openerCommands } from "@hypr/plugin-opener2";
import { Input } from "@hypr/ui/components/ui/input";
import {
  Popover,
  PopoverAnchor,
  PopoverContent,
} from "@hypr/ui/components/ui/popover";
import { cn } from "@hypr/utils";

import type { TodoProvider } from "./shared";

import { useAuth } from "~/auth";
import { useBillingAccess } from "~/auth/billing";
import { useConnections } from "~/auth/useConnections";
import { openIntegrationUrl } from "~/shared/integration";
import * as settings from "~/store/tinybase/store/settings";

async function searchGitHubRepos(query: string): Promise<string[]> {
  const resp = await fetch(
    `https://api.github.com/search/repositories?q=${encodeURIComponent(query)}&per_page=6`,
  );
  if (!resp.ok) {
    return [];
  }
  const data = (await resp.json()) as { items: { full_name: string }[] };
  return (data.items ?? []).map((item) => item.full_name);
}

export function GitHubTodoProviderContent({
  config,
}: {
  config: TodoProvider;
}) {
  const auth = useAuth();
  const { isPaid, upgradeToPro } = useBillingAccess();
  const { data: connections } = useConnections(isPaid);
  const [showAddInput, setShowAddInput] = useState(false);
  const [inputValue, setInputValue] = useState("");
  const [debouncedInput, setDebouncedInput] = useState("");
  const [showSuggestions, setShowSuggestions] = useState(false);

  const providerConnections = useMemo(
    () =>
      connections?.filter(
        (c) => c.integration_id === config.nangoIntegrationId,
      ) ?? [],
    [connections, config.nangoIntegrationId],
  );

  const repository =
    settings.UI.useValue("todo_github_repository", settings.STORE_ID) ?? "";
  const normalizedRepository = repository.trim();
  const hasRepository = normalizedRepository.length > 0;

  const setRepository = settings.UI.useSetValueCallback(
    "todo_github_repository",
    (value: string) => value,
    [],
    settings.STORE_ID,
  );

  useEffect(() => {
    const id = setTimeout(() => setDebouncedInput(inputValue), 300);
    return () => clearTimeout(id);
  }, [inputValue]);

  const { data: suggestions = [] } = useQuery({
    queryKey: ["github-repo-search", debouncedInput],
    queryFn: () => searchGitHubRepos(debouncedInput),
    enabled: debouncedInput.trim().length >= 2,
    staleTime: 30_000,
  });

  function handleSelect(repo: string) {
    setRepository(repo);
    setShowAddInput(false);
    setInputValue("");
    setDebouncedInput("");
    setShowSuggestions(false);
  }

  function handleAdd() {
    const trimmed = inputValue.trim();
    if (isGitHubRepository(trimmed)) {
      handleSelect(trimmed);
    }
  }

  const isValidInput = isGitHubRepository(inputValue.trim());
  const hasSuggestions = showSuggestions && suggestions.length > 0;

  return (
    <div className="flex flex-col gap-4">
      <p className="text-xs text-neutral-400">
        Only public repositories are supported.{" "}
        {!auth.session ? (
          <span>Sign in for private repo access.</span>
        ) : !isPaid ? (
          <button
            type="button"
            onClick={upgradeToPro}
            className="underline transition-colors hover:text-neutral-700"
          >
            Upgrade for private repos.
          </button>
        ) : providerConnections.length === 0 ? (
          <button
            type="button"
            onClick={() =>
              openIntegrationUrl(
                config.nangoIntegrationId,
                undefined,
                "connect",
                "todo",
              )
            }
            className="underline transition-colors hover:text-neutral-700"
          >
            Connect GitHub for private repos.
          </button>
        ) : (
          <button
            type="button"
            onClick={() =>
              openIntegrationUrl(
                config.nangoIntegrationId,
                providerConnections[0]?.connection_id,
                "disconnect",
                "todo",
              )
            }
            className="underline transition-colors hover:text-neutral-700"
          >
            Disconnect private repo access.
          </button>
        )}
      </p>

      {hasRepository && !showAddInput ? (
        <div className="flex items-center gap-2">
          <span className="text-sm text-neutral-700">
            {normalizedRepository}
          </span>
          <button
            type="button"
            onClick={() =>
              void openerCommands.openUrl(
                `https://github.com/${normalizedRepository}`,
                null,
              )
            }
            className="text-neutral-400 transition-colors hover:text-neutral-700"
          >
            <ExternalLinkIcon className="size-3.5" />
          </button>
          <button
            type="button"
            onClick={() => setRepository("")}
            className="text-neutral-400 transition-colors hover:text-neutral-700"
          >
            <XIcon className="size-3.5" />
          </button>
        </div>
      ) : null}

      {showAddInput ? (
        <Popover
          open={hasSuggestions}
          onOpenChange={(open) => !open && setShowSuggestions(false)}
        >
          <PopoverAnchor asChild>
            <form
              onSubmit={(e) => {
                e.preventDefault();
                handleAdd();
              }}
              className="flex items-center gap-2"
            >
              <Input
                autoFocus
                className="flex-1"
                placeholder="Search or type owner/repo"
                value={inputValue}
                onChange={(e) => {
                  setInputValue(e.target.value);
                  setShowSuggestions(true);
                }}
                onFocus={() => setShowSuggestions(true)}
              />
              <button
                type="submit"
                disabled={!isValidInput}
                className="text-xs text-neutral-600 underline transition-colors hover:text-neutral-900 disabled:cursor-not-allowed disabled:opacity-40"
              >
                Add
              </button>
              <button
                type="button"
                onClick={() => {
                  setShowAddInput(false);
                  setInputValue("");
                  setDebouncedInput("");
                }}
                className="text-xs text-neutral-400 underline transition-colors hover:text-neutral-600"
              >
                Cancel
              </button>
            </form>
          </PopoverAnchor>
          <PopoverContent
            className="p-1"
            align="start"
            onOpenAutoFocus={(e) => e.preventDefault()}
          >
            {suggestions.map((repo) => (
              <button
                key={repo}
                type="button"
                onMouseDown={(e) => e.preventDefault()}
                onClick={() => handleSelect(repo)}
                className={cn([
                  "flex w-full items-center px-3 py-1.5 text-left text-sm text-neutral-700",
                  "transition-colors hover:bg-neutral-50",
                ])}
              >
                {repo}
              </button>
            ))}
          </PopoverContent>
        </Popover>
      ) : (
        <button
          type="button"
          onClick={() => setShowAddInput(true)}
          className="flex w-fit items-center gap-1 text-xs text-neutral-500 transition-colors hover:text-neutral-800"
        >
          <PlusIcon className="size-3" />
          {hasRepository ? "Replace repository" : "Add repository"}
        </button>
      )}
    </div>
  );
}

function isGitHubRepository(value: string) {
  return /^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/.test(value);
}

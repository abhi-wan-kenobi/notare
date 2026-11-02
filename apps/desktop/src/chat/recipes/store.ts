import {
  QueryClient,
  useMutation,
  useQuery,
  useQueryClient,
} from "@tanstack/react-query";
import { useCallback, useState } from "react";

import {
  deleteRecipe,
  listRecipes,
  type Recipe,
  upsertRecipe,
  type UpsertRecipeInput,
} from "@hypr/plugin-db";

import { BUILTIN_RECIPES } from "./defaults";
import type { RecipeDraft } from "./types";

export const RECIPES_QUERY_KEY = ["chat-recipes"] as const;

const fallbackQueryClient = new QueryClient({
  defaultOptions: {
    queries: {
      retry: false,
    },
  },
});

function useSafeQueryClient(): QueryClient {
  try {
    return useQueryClient();
  } catch {
    return fallbackQueryClient;
  }
}

/**
 * Ensures built-in recipes are seeded into SQLite.
 * Runs in app code as requested: seeds Action items, Key decisions,
 * Follow-up email draft, Customer objections, Weekly digest, Questions I didn't ask.
 */
async function fetchAndSeedRecipes(): Promise<Recipe[]> {
  if (typeof listRecipes !== "function") {
    return BUILTIN_RECIPES as Recipe[];
  }
  let existing: Recipe[];
  try {
    existing = await listRecipes();
  } catch {
    return BUILTIN_RECIPES as Recipe[];
  }
  const existingIds = new Set(existing.map((r) => r.id));
  const missingBuiltins = BUILTIN_RECIPES.filter((b) => !existingIds.has(b.id));
  if (missingBuiltins.length > 0) {
    if (typeof upsertRecipe !== "function") {
      return BUILTIN_RECIPES as Recipe[];
    }
    for (const builtin of missingBuiltins) {
      try {
        const seeded = await upsertRecipe({
          id: builtin.id,
          title: builtin.title,
          prompt: builtin.prompt,
          scope: builtin.scope,
          builtin: true,
        });
        existing.push(seeded);
      } catch {
        // Fallback: use builtin in memory
        existing.push({
          ...builtin,
          createdAt: new Date().toISOString(),
          updatedAt: new Date().toISOString(),
        });
      }
    }
    // Re-sort: built-ins first, then title
    existing.sort((a, b) => {
      if (a.builtin !== b.builtin) {
        return a.builtin ? -1 : 1;
      }
      return a.title.localeCompare(b.title);
    });
  }

  return existing;
}

export function useRecipes() {
  const queryClient = useSafeQueryClient();

  const query = useQuery(
    {
      queryKey: RECIPES_QUERY_KEY,
      queryFn: fetchAndSeedRecipes,
    },
    queryClient,
  );

  const createMutation = useMutation(
    {
      mutationFn: async (draft: RecipeDraft) => {
        const input: UpsertRecipeInput = {
          id: draft.id || crypto.randomUUID(),
          title: draft.title.trim(),
          prompt: draft.prompt.trim(),
          scope: draft.scope,
          builtin: Boolean(draft.builtin),
        };
        return upsertRecipe(input);
      },
      onSuccess: () => {
        void queryClient.invalidateQueries({ queryKey: RECIPES_QUERY_KEY });
      },
    },
    queryClient,
  );

  const updateMutation = useMutation(
    {
      mutationFn: async (recipe: UpsertRecipeInput) => {
        return upsertRecipe(recipe);
      },
      onSuccess: () => {
        void queryClient.invalidateQueries({ queryKey: RECIPES_QUERY_KEY });
      },
    },
    queryClient,
  );

  const duplicateMutation = useMutation(
    {
      mutationFn: async (recipe: Recipe) => {
        const input: UpsertRecipeInput = {
          id: crypto.randomUUID(),
          title: `${recipe.title} (Copy)`,
          prompt: recipe.prompt,
          scope: recipe.scope,
          builtin: false,
        };
        return upsertRecipe(input);
      },
      onSuccess: () => {
        void queryClient.invalidateQueries({ queryKey: RECIPES_QUERY_KEY });
      },
    },
    queryClient,
  );

  const deleteMutation = useMutation(
    {
      mutationFn: async (id: string) => {
        return deleteRecipe(id);
      },
      onSuccess: () => {
        void queryClient.invalidateQueries({ queryKey: RECIPES_QUERY_KEY });
      },
    },
    queryClient,
  );

  return {
    recipes: query.data ?? (BUILTIN_RECIPES as Recipe[]),
    isLoading: query.isLoading,
    error: query.error,
    refetch: query.refetch,
    createRecipe: createMutation.mutateAsync,
    updateRecipe: updateMutation.mutateAsync,
    duplicateRecipe: duplicateMutation.mutateAsync,
    deleteRecipe: deleteMutation.mutateAsync,
  };
}

// In-memory active recipe map keyed by draftKey (e.g. sessionId)
const activeRecipeByDraftKey = new Map<string, Recipe | null>();

export function useRecipeDraftState({
  draftKey,
  onActiveRecipeChange,
}: {
  draftKey: string;
  onActiveRecipeChange?: (recipe: Recipe | null) => void;
}) {
  const [activeRecipe, setActiveRecipeState] = useState<Recipe | null>(
    () => activeRecipeByDraftKey.get(draftKey) ?? null,
  );

  const setActiveRecipe = useCallback(
    (recipe: Recipe | null) => {
      setActiveRecipeState(recipe);
      if (recipe) {
        activeRecipeByDraftKey.set(draftKey, recipe);
      } else {
        activeRecipeByDraftKey.delete(draftKey);
      }
      onActiveRecipeChange?.(recipe);
    },
    [draftKey, onActiveRecipeChange],
  );

  const clearActiveRecipe = useCallback(() => {
    setActiveRecipeState(null);
    activeRecipeByDraftKey.delete(draftKey);
    onActiveRecipeChange?.(null);
  }, [draftKey, onActiveRecipeChange]);

  return {
    activeRecipe,
    setActiveRecipe,
    clearActiveRecipe,
  };
}

export function clearRecipeDraft(draftKey: string) {
  activeRecipeByDraftKey.delete(draftKey);
}

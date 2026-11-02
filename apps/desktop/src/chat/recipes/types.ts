import type { Recipe, UpsertRecipeInput } from "@hypr/plugin-db";

export type { Recipe, UpsertRecipeInput };

export type RecipeScope = "single" | "multi";

export interface RecipeDraft {
  id?: string;
  title: string;
  prompt: string;
  scope: RecipeScope;
  builtin?: boolean;
}

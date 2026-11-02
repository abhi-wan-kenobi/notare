import { Trans, useLingui } from "@lingui/react/macro";
import {
  CopyIcon,
  PencilIcon,
  PlusIcon,
  SearchIcon,
  SparklesIcon,
  Trash2Icon,
} from "lucide-react";
import { useMemo, useState } from "react";

import { Button } from "@hypr/ui/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@hypr/ui/components/ui/dialog";
import { Input } from "@hypr/ui/components/ui/input";
import { Textarea } from "@hypr/ui/components/ui/textarea";
import { cn } from "@hypr/utils";

import {
  filterRecipes,
  type Recipe,
  type RecipeDraft,
  type RecipeScope,
  useRecipes,
} from "~/chat/recipes";
import { SettingsPageTitle } from "~/settings/page-title";

export function SettingsRecipes() {
  const { t } = useLingui();
  const {
    recipes,
    isLoading,
    createRecipe,
    updateRecipe,
    duplicateRecipe,
    deleteRecipe,
  } = useRecipes();

  const [searchQuery, setSearchQuery] = useState("");
  const [editingRecipe, setEditingRecipe] = useState<Recipe | null>(null);
  const [isCreating, setIsCreating] = useState(false);
  const [deleteConfirmRecipe, setDeleteConfirmRecipe] = useState<Recipe | null>(
    null,
  );

  const filtered = useMemo(
    () => filterRecipes(recipes, searchQuery),
    [recipes, searchQuery],
  );

  const handleDuplicate = async (recipe: Recipe) => {
    try {
      await duplicateRecipe(recipe);
    } catch (err) {
      console.error("[settings/recipes] Failed to duplicate recipe:", err);
    }
  };

  const handleDelete = async (recipe: Recipe) => {
    if (recipe.builtin) {
      return;
    }
    try {
      await deleteRecipe(recipe.id);
      setDeleteConfirmRecipe(null);
    } catch (err) {
      console.error("[settings/recipes] Failed to delete recipe:", err);
    }
  };

  return (
    <div className="flex flex-col gap-6">
      <div className="flex flex-col gap-1">
        <SettingsPageTitle title={t`Recipes`} />
        <p className="text-muted-foreground text-xs">
          <Trans>Prompt templates for chat workflows and analysis.</Trans>
        </p>
      </div>

      <div className="flex items-center justify-between gap-3">
        <div className="relative max-w-sm flex-1">
          <SearchIcon className="text-muted-foreground absolute top-1/2 left-2.5 size-4 -translate-y-1/2" />
          <Input
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            placeholder={t`Search recipes...`}
            className="pl-9 text-xs"
          />
        </div>
        <Button
          size="sm"
          onClick={() => setIsCreating(true)}
          className="flex items-center gap-1.5 text-xs"
        >
          <PlusIcon className="size-3.5" />
          <Trans>New recipe</Trans>
        </Button>
      </div>

      {isLoading ? (
        <div className="text-muted-foreground py-8 text-center text-xs">
          <Trans>Loading recipes...</Trans>
        </div>
      ) : filtered.length === 0 ? (
        <div className="border-border/60 bg-card/40 flex flex-col items-center justify-center rounded-xl border py-12 text-center">
          <SparklesIcon className="text-muted-foreground/50 mb-2 size-8" />
          <span className="text-muted-foreground text-xs">
            {searchQuery
              ? t`No recipes match your search.`
              : t`No recipes yet.`}
          </span>
        </div>
      ) : (
        <div className="flex flex-col gap-2">
          {filtered.map((recipe) => (
            <div
              key={recipe.id}
              data-recipe-card={recipe.id}
              className="border-border/60 bg-card/60 hover:bg-card/90 flex flex-col gap-2 rounded-xl border p-4 transition-colors"
            >
              <div className="flex items-start justify-between gap-3">
                <div className="flex flex-col gap-1">
                  <div className="flex items-center gap-2">
                    <span className="text-sm font-semibold">
                      {recipe.title}
                    </span>
                    {recipe.builtin && (
                      <span className="border-border/60 bg-muted/60 text-muted-foreground rounded-md border px-1.5 py-0.5 text-[10px] font-medium">
                        <Trans>Built-in</Trans>
                      </span>
                    )}
                    <span
                      className={cn([
                        "rounded-md border px-1.5 py-0.5 text-[10px] font-medium",
                        recipe.scope === "multi"
                          ? "border-amber-500/20 bg-amber-500/10 text-amber-500"
                          : "border-border/60 bg-muted/60 text-muted-foreground",
                      ])}
                    >
                      {recipe.scope === "multi"
                        ? t`Multi-meeting`
                        : t`Single-meeting`}
                    </span>
                  </div>
                </div>

                <div className="flex items-center gap-1">
                  <Button
                    size="icon"
                    variant="ghost"
                    className="size-7"
                    onClick={() => handleDuplicate(recipe)}
                    title={t`Duplicate recipe`}
                    aria-label={t`Duplicate recipe`}
                  >
                    <CopyIcon className="size-3.5" />
                  </Button>
                  <Button
                    size="icon"
                    variant="ghost"
                    className="size-7"
                    onClick={() => setEditingRecipe(recipe)}
                    title={t`Edit recipe`}
                    aria-label={t`Edit recipe`}
                  >
                    <PencilIcon className="size-3.5" />
                  </Button>
                  {!recipe.builtin && (
                    <Button
                      size="icon"
                      variant="ghost"
                      className="text-destructive hover:text-destructive size-7"
                      onClick={() => setDeleteConfirmRecipe(recipe)}
                      title={t`Delete recipe`}
                      aria-label={t`Delete recipe`}
                    >
                      <Trash2Icon className="size-3.5" />
                    </Button>
                  )}
                </div>
              </div>

              <div className="text-muted-foreground bg-muted/30 border-border/40 line-clamp-3 rounded-lg border p-2 font-mono text-xs whitespace-pre-wrap">
                {recipe.prompt}
              </div>
            </div>
          ))}
        </div>
      )}

      {/* Add Recipe Modal */}
      {isCreating && (
        <RecipeFormDialog
          open={isCreating}
          title={t`New recipe`}
          onClose={() => setIsCreating(false)}
          onSave={async (draft) => {
            await createRecipe(draft);
            setIsCreating(false);
          }}
        />
      )}

      {/* Edit Recipe Modal */}
      {editingRecipe && (
        <RecipeFormDialog
          open={Boolean(editingRecipe)}
          title={t`Edit recipe`}
          initial={editingRecipe}
          onClose={() => setEditingRecipe(null)}
          onSave={async (draft) => {
            await updateRecipe({
              id: editingRecipe.id,
              title: draft.title,
              prompt: draft.prompt,
              scope: draft.scope,
              builtin: editingRecipe.builtin,
            });
            setEditingRecipe(null);
          }}
        />
      )}

      {/* Delete Confirmation Modal */}
      {deleteConfirmRecipe && (
        <Dialog
          open={Boolean(deleteConfirmRecipe)}
          onOpenChange={(open) => !open && setDeleteConfirmRecipe(null)}
        >
          <DialogContent className="max-w-md">
            <DialogHeader>
              <DialogTitle>
                <Trans>Delete recipe</Trans>
              </DialogTitle>
              <DialogDescription>
                <Trans>
                  Are you sure you want to delete "{deleteConfirmRecipe.title}"?
                  This action cannot be undone.
                </Trans>
              </DialogDescription>
            </DialogHeader>
            <DialogFooter className="gap-2">
              <Button
                variant="outline"
                size="sm"
                onClick={() => setDeleteConfirmRecipe(null)}
              >
                <Trans>Cancel</Trans>
              </Button>
              <Button
                variant="destructive"
                size="sm"
                onClick={() => handleDelete(deleteConfirmRecipe)}
              >
                <Trans>Delete</Trans>
              </Button>
            </DialogFooter>
          </DialogContent>
        </Dialog>
      )}
    </div>
  );
}

function RecipeFormDialog({
  open,
  title,
  initial,
  onClose,
  onSave,
}: {
  open: boolean;
  title: string;
  initial?: Recipe;
  onClose: () => void;
  onSave: (draft: RecipeDraft) => Promise<void>;
}) {
  const { t } = useLingui();
  const [recipeTitle, setRecipeTitle] = useState(initial?.title ?? "");
  const [prompt, setPrompt] = useState(initial?.prompt ?? "");
  const [scope, setScope] = useState<RecipeScope>(
    initial?.scope === "multi" ? "multi" : "single",
  );
  const [isSubmitting, setIsSubmitting] = useState(false);

  const canSave = recipeTitle.trim().length > 0 && prompt.trim().length > 0;

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!canSave || isSubmitting) return;

    setIsSubmitting(true);
    try {
      await onSave({
        title: recipeTitle.trim(),
        prompt: prompt.trim(),
        scope,
      });
    } finally {
      setIsSubmitting(false);
    }
  };

  return (
    <Dialog open={open} onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-w-lg">
        <form onSubmit={handleSubmit} className="flex flex-col gap-4">
          <DialogHeader>
            <DialogTitle>{title}</DialogTitle>
            <DialogDescription>
              <Trans>
                Configure the title, scope, and prompt template for this recipe.
              </Trans>
            </DialogDescription>
          </DialogHeader>

          <div className="flex flex-col gap-3">
            <div className="flex flex-col gap-1.5">
              <label className="text-foreground text-xs font-medium">
                <Trans>Title</Trans>
              </label>
              <Input
                value={recipeTitle}
                onChange={(e) => setRecipeTitle(e.target.value)}
                placeholder={t`e.g. Action items, Executive summary...`}
                className="text-xs"
                autoFocus
              />
            </div>

            <div className="flex flex-col gap-1.5">
              <label className="text-foreground text-xs font-medium">
                <Trans>Scope</Trans>
              </label>
              <div className="flex items-center gap-2">
                <button
                  type="button"
                  onClick={() => setScope("single")}
                  className={cn([
                    "flex-1 rounded-lg border px-3 py-2 text-xs font-medium transition-colors",
                    scope === "single"
                      ? "border-primary bg-primary/10 text-primary"
                      : "border-border/60 hover:bg-muted/60 text-muted-foreground",
                  ])}
                >
                  <Trans>Single meeting</Trans>
                </button>
                <button
                  type="button"
                  onClick={() => setScope("multi")}
                  className={cn([
                    "flex-1 rounded-lg border px-3 py-2 text-xs font-medium transition-colors",
                    scope === "multi"
                      ? "border-primary bg-primary/10 text-primary"
                      : "border-border/60 hover:bg-muted/60 text-muted-foreground",
                  ])}
                >
                  <Trans>Multi-meeting</Trans>
                </button>
              </div>
            </div>

            <div className="flex flex-col gap-1.5">
              <div className="flex items-center justify-between">
                <label className="text-foreground text-xs font-medium">
                  <Trans>Prompt template</Trans>
                </label>
                <span className="text-muted-foreground text-[10px]">
                  <Trans>Supports Markdown and</Trans> {"{{selection}}"}
                </span>
              </div>
              <Textarea
                value={prompt}
                onChange={(e) => setPrompt(e.target.value)}
                placeholder={t`Enter the instruction prompt for the model... (Use {{selection}} to insert selected text)`}
                rows={6}
                className="font-mono text-xs"
              />
            </div>
          </div>

          <DialogFooter className="gap-2">
            <Button
              type="button"
              variant="outline"
              size="sm"
              onClick={onClose}
              disabled={isSubmitting}
            >
              <Trans>Cancel</Trans>
            </Button>
            <Button type="submit" size="sm" disabled={!canSave || isSubmitting}>
              <Trans>Save</Trans>
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}

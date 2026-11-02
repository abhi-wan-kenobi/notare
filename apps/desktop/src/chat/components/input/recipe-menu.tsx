import { SparklesIcon } from "lucide-react";

import { cn } from "@hypr/utils";

import type { Recipe } from "~/chat/recipes";

export function RecipeSlashMenu({
  id = "recipe-slash-menu-listbox",
  items,
  selectedIndex,
  onSelect,
  onHoverIndex,
}: {
  id?: string;
  items: Recipe[];
  selectedIndex: number;
  onSelect: (recipe: Recipe) => void;
  onHoverIndex: (index: number) => void;
}) {
  if (items.length === 0) {
    return null;
  }

  return (
    <div
      id={id}
      data-recipe-slash-menu
      role="listbox"
      aria-label="Recipes"
      className={cn([
        "border-border bg-popover/95 text-popover-foreground absolute bottom-full left-0 z-50 mb-2 max-h-64 w-80 overflow-y-auto rounded-lg border p-1 shadow-lg backdrop-blur-sm",
      ])}
    >
      <div className="text-muted-foreground px-2 py-1 text-[10px] font-semibold tracking-wider uppercase select-none">
        Recipes
      </div>
      {items.map((recipe, index) => {
        const isSelected = index === selectedIndex;
        return (
          <button
            key={recipe.id}
            id={`recipe-option-${recipe.id}`}
            role="option"
            aria-selected={isSelected}
            type="button"
            className={cn([
              "flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-xs transition-colors",
              isSelected
                ? "bg-accent text-accent-foreground"
                : "hover:bg-muted/60 text-foreground",
            ])}
            onClick={() => onSelect(recipe)}
            onMouseEnter={() => onHoverIndex(index)}
          >
            <div className="border-border bg-muted flex size-6 shrink-0 items-center justify-center rounded border">
              <SparklesIcon className="text-muted-foreground size-3" />
            </div>
            <div className="flex min-w-0 flex-1 flex-col">
              <div className="flex items-center gap-1.5">
                <span className="truncate font-medium">{recipe.title}</span>
                {recipe.scope === "multi" && (
                  <span className="border-border/60 bg-muted/60 text-muted-foreground rounded border px-1 text-[9px]">
                    multi
                  </span>
                )}
                {recipe.builtin && (
                  <span className="text-muted-foreground text-[9px]">
                    built-in
                  </span>
                )}
              </div>
              <span className="text-muted-foreground truncate text-[11px]">
                {recipe.prompt.slice(0, 60)}
              </span>
            </div>
          </button>
        );
      })}
    </div>
  );
}

export function RecipeChip({
  recipe,
  onRemove,
}: {
  recipe: Recipe;
  onRemove: () => void;
}) {
  return (
    <div data-recipe-chip className="flex items-center gap-1.5 px-2 pt-1 pb-1">
      <span className="border-border/60 bg-muted/70 text-foreground inline-flex h-6 items-center gap-1.5 rounded-md border px-2 text-xs font-medium shadow-2xs">
        <SparklesIcon className="text-muted-foreground size-3 shrink-0" />
        <span className="max-w-[200px] truncate">{recipe.title}</span>
        {recipe.scope === "multi" && (
          <span className="text-muted-foreground text-[10px] uppercase">
            (multi)
          </span>
        )}
        <button
          type="button"
          onClick={onRemove}
          className="hover:text-foreground text-muted-foreground ml-0.5 rounded-xs p-0.5"
          title="Remove recipe"
          aria-label="Remove recipe"
        >
          <svg
            className="size-3"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
            strokeLinecap="round"
            strokeLinejoin="round"
          >
            <line x1="18" y1="6" x2="6" y2="18" />
            <line x1="6" y1="6" x2="18" y2="18" />
          </svg>
        </button>
      </span>
    </div>
  );
}

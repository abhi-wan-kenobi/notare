import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type * as ChatRecipesModule from "~/chat/recipes";
import type { Recipe, RecipeDraft, UpsertRecipeInput } from "~/chat/recipes";

const initialRecipes: Recipe[] = [
  {
    id: "builtin-action-items",
    title: "Action items",
    prompt: "Extract tasks and deadlines",
    scope: "single",
    builtin: true,
    createdAt: "2026-01-01T00:00:00Z",
    updatedAt: "2026-01-01T00:00:00Z",
  },
  {
    id: "user-recipe-1",
    title: "My Custom Summary",
    prompt: "Custom meeting summary",
    scope: "single",
    builtin: false,
    createdAt: "2026-01-02T00:00:00Z",
    updatedAt: "2026-01-02T00:00:00Z",
  },
];

let mockDbRecipes: Recipe[] = [];

const mocks = vi.hoisted(() => ({
  createRecipe: vi.fn(),
  updateRecipe: vi.fn(),
  duplicateRecipe: vi.fn(),
  deleteRecipe: vi.fn(),
}));

vi.mock("~/chat/recipes", async (importOriginal) => {
  const actual = await importOriginal<typeof ChatRecipesModule>();
  return {
    ...actual,
    useRecipes: () => ({
      recipes: mockDbRecipes,
      isLoading: false,
      error: null,
      refetch: vi.fn(),
      createRecipe: mocks.createRecipe,
      updateRecipe: mocks.updateRecipe,
      duplicateRecipe: mocks.duplicateRecipe,
      deleteRecipe: mocks.deleteRecipe,
    }),
  };
});

vi.mock("@lingui/react/macro", () => ({
  Trans: ({ children }: { children?: ReactNode }) => <>{children}</>,
  useLingui: () => ({
    t: (input: TemplateStringsArray | string, ...values: unknown[]) => {
      if (typeof input === "string") {
        return input;
      }
      return (input as readonly string[]).reduce(
        (message: string, part: string, index: number) =>
          `${message}${part}${index < values.length ? String(values[index]) : ""}`,
        "",
      );
    },
  }),
}));

import { SettingsRecipes } from "./index";

describe("SettingsRecipes", () => {
  beforeEach(() => {
    cleanup();
    mockDbRecipes = JSON.parse(JSON.stringify(initialRecipes));
    mocks.createRecipe.mockImplementation(async (draft: RecipeDraft) => {
      const created: Recipe = {
        id: "new-recipe-id",
        title: draft.title,
        prompt: draft.prompt,
        scope: draft.scope,
        builtin: false,
        createdAt: new Date().toISOString(),
        updatedAt: new Date().toISOString(),
      };
      mockDbRecipes.push(created);
      return created;
    });
    mocks.updateRecipe.mockImplementation(async (input: UpsertRecipeInput) => {
      const idx = mockDbRecipes.findIndex((r) => r.id === input.id);
      const updated: Recipe = {
        id: input.id,
        title: input.title,
        prompt: input.prompt,
        scope: input.scope,
        builtin: Boolean(input.builtin),
        createdAt: "2026-01-01T00:00:00Z",
        updatedAt: new Date().toISOString(),
      };
      if (idx >= 0) {
        mockDbRecipes[idx] = updated;
      }
      return updated;
    });
    mocks.duplicateRecipe.mockImplementation(async (recipe: Recipe) => {
      const copy: Recipe = {
        id: "copied-recipe-id",
        title: `${recipe.title} (Copy)`,
        prompt: recipe.prompt,
        scope: recipe.scope,
        builtin: false,
        createdAt: new Date().toISOString(),
        updatedAt: new Date().toISOString(),
      };
      mockDbRecipes.push(copy);
      return copy;
    });
    mocks.deleteRecipe.mockImplementation(async (id: string) => {
      mockDbRecipes = mockDbRecipes.filter((r) => r.id !== id);
    });
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it("lists existing recipes including built-in and user recipes", () => {
    render(<SettingsRecipes />);

    expect(screen.getByText("Action items")).toBeTruthy();
    expect(screen.getByText("My Custom Summary")).toBeTruthy();
    expect(screen.getByText("Built-in")).toBeTruthy();
  });

  it("filters recipes by search query", () => {
    render(<SettingsRecipes />);

    const searchInput = screen.getByPlaceholderText(/search recipes/i);
    fireEvent.change(searchInput, { target: { value: "Custom" } });

    expect(screen.getByText("My Custom Summary")).toBeTruthy();
    expect(screen.queryByText("Action items")).toBeNull();
  });

  it("adds a new user recipe", async () => {
    render(<SettingsRecipes />);

    // Open New recipe dialog
    fireEvent.click(screen.getByText("New recipe"));

    // Enter title and prompt
    const titleInput = screen.getByPlaceholderText(/e\.g\. Action items/i);
    const promptInput = screen.getByPlaceholderText(/enter the instruction/i);

    fireEvent.change(titleInput, { target: { value: "Standup Notes" } });
    fireEvent.change(promptInput, {
      target: { value: "Generate daily standup bullet points" },
    });

    // Click Save
    const saveButton = screen.getByRole("button", { name: "Save" });
    fireEvent.click(saveButton);

    await waitFor(() => {
      expect(mocks.createRecipe).toHaveBeenCalledWith({
        title: "Standup Notes",
        prompt: "Generate daily standup bullet points",
        scope: "single",
      });
    });
  });

  it("edits an existing recipe", async () => {
    render(<SettingsRecipes />);

    // Find the edit button for "My Custom Summary"
    const card = screen
      .getByText("My Custom Summary")
      .closest("[data-recipe-card]")!;
    const editButton = card.querySelector('button[aria-label="Edit recipe"]')!;

    fireEvent.click(editButton);

    // Dialog opens with current title
    const titleInput = screen.getByDisplayValue("My Custom Summary");
    fireEvent.change(titleInput, { target: { value: "Updated Summary" } });

    // Click Save
    fireEvent.click(screen.getByRole("button", { name: "Save" }));

    await waitFor(() => {
      expect(mocks.updateRecipe).toHaveBeenCalledWith(
        expect.objectContaining({
          id: "user-recipe-1",
          title: "Updated Summary",
          builtin: false,
        }),
      );
    });
  });

  it("duplicates a recipe", async () => {
    render(<SettingsRecipes />);

    // Find the duplicate button on Action items card
    const card = screen
      .getByText("Action items")
      .closest("[data-recipe-card]")!;
    const duplicateButton = card.querySelector(
      'button[aria-label="Duplicate recipe"]',
    )!;

    fireEvent.click(duplicateButton);

    await waitFor(() => {
      expect(mocks.duplicateRecipe).toHaveBeenCalledWith(
        expect.objectContaining({
          title: "Action items",
          prompt: "Extract tasks and deadlines",
          scope: "single",
          builtin: true,
        }),
      );
    });
  });

  it("does not show delete button for built-in recipes", () => {
    render(<SettingsRecipes />);

    const builtinCard = screen
      .getByText("Action items")
      .closest("[data-recipe-card]")!;
    const deleteBtn = builtinCard.querySelector(
      'button[aria-label="Delete recipe"]',
    );
    expect(deleteBtn).toBeNull();
  });

  it("deletes a user recipe after confirmation", async () => {
    render(<SettingsRecipes />);

    const userCard = screen
      .getByText("My Custom Summary")
      .closest("[data-recipe-card]")!;
    const deleteBtn = userCard.querySelector(
      'button[aria-label="Delete recipe"]',
    )!;

    // Click delete button
    fireEvent.click(deleteBtn);

    // Confirmation dialog should be visible
    expect(screen.getByText(/Are you sure you want to delete/i)).toBeTruthy();

    // Confirm deletion
    const confirmDeleteBtn = screen.getByRole("button", { name: "Delete" });
    fireEvent.click(confirmDeleteBtn);

    await waitFor(() => {
      expect(mocks.deleteRecipe).toHaveBeenCalledWith("user-recipe-1");
    });
  });
});

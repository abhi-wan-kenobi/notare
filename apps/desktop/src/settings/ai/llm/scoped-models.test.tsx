import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { ProviderStatus } from "./select";

// `ModelCombobox` is a Popover+cmdk combobox - its own interaction is
// exercised elsewhere; here we only care that ScopedModelRow wires its
// `onChange` straight into `onChangeModel`, so a minimal stub keeps these
// tests about the row's own logic instead of Radix Popover internals.
vi.mock("~/settings/ai/shared/model-combobox", () => ({
  ModelCombobox: ({
    providerId,
    value,
    onChange,
  }: {
    providerId: string;
    value: string;
    onChange: (value: string) => void;
  }) => (
    <button
      type="button"
      aria-label={`model-combobox-${providerId}`}
      onClick={() => onChange("chosen-model")}
    >
      {value || "Select a model"}
    </button>
  ),
}));

const mocks = vi.hoisted(() => ({
  configuredProviders: {} as Record<string, ProviderStatus>,
  configValues: {} as Record<string, string>,
  setValues: vi.fn(),
}));

vi.mock("./select", async (importOriginal) => {
  const actual = await importOriginal<typeof import("./select")>();
  return {
    ...actual,
    useConfiguredMapping: () => mocks.configuredProviders,
  };
});

vi.mock("~/shared/config", () => ({
  useConfigValues: () => mocks.configValues,
}));

vi.mock("~/settings/queries", () => ({
  useSetSettingValues: () => mocks.setValues,
}));

// `ScopedModelSettings` also reads the *global* provider's base URL (to
// decide whether cloud is opted into globally, mirroring
// `~/ai/scope.ts`'s `resolveScopeSelection` invariant) via `useAiProvider`,
// which otherwise needs a live DB query + QueryClientProvider. Stubbed out
// since these tests don't exercise that base-URL lookup.
vi.mock("~/settings/providers", () => ({
  useAiProvider: () => undefined,
}));

import { ScopedModelRow, ScopedModelSettings } from "./scoped-models";

const CONFIGURED: Record<string, ProviderStatus> = {
  ollama: {
    configured: true,
    listModels: async () => ({ models: [], ignored: [], metadata: {} }),
  },
  openai: {
    configured: true,
    listModels: async () => ({ models: [], ignored: [], metadata: {} }),
  },
  anthropic: { configured: false },
};

describe("ScopedModelRow", () => {
  afterEach(() => {
    cleanup();
  });

  function renderRow(
    overrides: Partial<Parameters<typeof ScopedModelRow>[0]> = {},
  ) {
    const onChangeProvider = vi.fn();
    const onChangeModel = vi.fn();
    const onUseDefault = vi.fn();
    render(
      <ScopedModelRow
        label="Cleanup (dictation)"
        ariaLabel="Cleanup (dictation) model"
        providerId=""
        modelId=""
        configuredProviders={CONFIGURED}
        onChangeProvider={onChangeProvider}
        onChangeModel={onChangeModel}
        onUseDefault={onUseDefault}
        {...overrides}
      />,
    );
    return { onChangeProvider, onChangeModel, onUseDefault };
  }

  it('shows "Use default model" when both keys are empty, and hides the model picker', () => {
    renderRow();

    expect(screen.getByText("Use default model")).toBeTruthy();
    expect(screen.queryByLabelText(/model-combobox-/)).toBeNull();
  });

  it("hides configured cloud providers when cloud isn't opted into globally, keeps local ones", () => {
    renderRow({ globalIsCloud: false });

    fireEvent.click(screen.getByRole("combobox"));
    expect(screen.getByText("Ollama")).toBeTruthy();
    expect(screen.queryByText("OpenAI")).toBeNull();
  });

  it("offers configured cloud providers once cloud is opted into globally", () => {
    renderRow({ globalIsCloud: true });

    fireEvent.click(screen.getByRole("combobox"));
    expect(screen.getByText("Ollama")).toBeTruthy();
    expect(screen.getByText("OpenAI")).toBeTruthy();
  });

  it("only offers already-configured providers, never an unconfigured one", () => {
    renderRow();

    fireEvent.click(screen.getByRole("combobox"));
    expect(screen.getByText("Ollama")).toBeTruthy();
    expect(screen.getByText("OpenAI")).toBeTruthy();
    expect(screen.queryByText("Anthropic")).toBeNull();
  });

  it("selecting a provider clears the model and reports the new provider", () => {
    const { onChangeProvider } = renderRow();

    fireEvent.click(screen.getByRole("combobox"));
    fireEvent.click(screen.getByText("Ollama"));

    expect(onChangeProvider).toHaveBeenCalledWith("ollama");
  });

  it("shows the model picker once a provider is set, and reports the picked model", () => {
    const { onChangeModel } = renderRow({ providerId: "ollama", modelId: "" });

    const combobox = screen.getByLabelText("model-combobox-ollama");
    fireEvent.click(combobox);
    expect(onChangeModel).toHaveBeenCalledWith("chosen-model");
  });

  it('picking "Use default model" again clears back to the default', () => {
    const { onUseDefault } = renderRow({
      providerId: "ollama",
      modelId: "llama3",
    });

    fireEvent.click(screen.getByRole("combobox"));
    fireEvent.click(screen.getByText("Use default model"));

    expect(onUseDefault).toHaveBeenCalledTimes(1);
  });
});

describe("ScopedModelSettings", () => {
  afterEach(() => {
    cleanup();
    mocks.setValues.mockReset();
    mocks.configuredProviders = {};
    mocks.configValues = {};
  });

  it("renders one row per scope, all defaulting to 'Use default model' when unset", () => {
    mocks.configuredProviders = {
      ollama: {
        configured: true,
        listModels: async () => ({ models: [], ignored: [], metadata: {} }),
      },
    };
    mocks.configValues = {
      ai_scope_cleanup_provider: "",
      ai_scope_cleanup_model: "",
      ai_scope_notes_provider: "",
      ai_scope_notes_model: "",
      ai_scope_chat_provider: "",
      ai_scope_chat_model: "",
    };

    render(<ScopedModelSettings />);

    expect(screen.getAllByText("Use default model")).toHaveLength(3);
  });

  it("picking a provider then a model writes both keys for that scope", () => {
    mocks.configuredProviders = {
      ollama: {
        configured: true,
        listModels: async () => ({ models: [], ignored: [], metadata: {} }),
      },
    };
    mocks.configValues = {
      ai_scope_cleanup_provider: "",
      ai_scope_cleanup_model: "",
      ai_scope_notes_provider: "",
      ai_scope_notes_model: "",
      ai_scope_chat_provider: "",
      ai_scope_chat_model: "",
    };

    render(<ScopedModelSettings />);

    const [cleanupCombobox] = screen.getAllByRole("combobox");
    fireEvent.click(cleanupCombobox);
    fireEvent.click(screen.getByText("Ollama"));

    expect(mocks.setValues).toHaveBeenCalledWith({
      ai_scope_cleanup_provider: "ollama",
      ai_scope_cleanup_model: "",
    });
  });

  it('picking "Use default model" for a scope clears both of its keys', () => {
    mocks.configuredProviders = {
      ollama: {
        configured: true,
        listModels: async () => ({ models: [], ignored: [], metadata: {} }),
      },
    };
    mocks.configValues = {
      ai_scope_cleanup_provider: "ollama",
      ai_scope_cleanup_model: "llama3",
      ai_scope_notes_provider: "",
      ai_scope_notes_model: "",
      ai_scope_chat_provider: "",
      ai_scope_chat_model: "",
    };

    render(<ScopedModelSettings />);

    const [cleanupCombobox] = screen.getAllByRole("combobox");
    fireEvent.click(cleanupCombobox);
    fireEvent.click(screen.getByRole("option", { name: "Use default model" }));

    expect(mocks.setValues).toHaveBeenCalledWith({
      ai_scope_cleanup_provider: "",
      ai_scope_cleanup_model: "",
    });
  });
});

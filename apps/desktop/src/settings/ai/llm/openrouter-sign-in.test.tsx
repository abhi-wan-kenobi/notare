import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { OpenRouterSignIn } from "./openrouter-sign-in";

const mocks = vi.hoisted(() => ({
  openrouterSignIn: vi.fn(),
  openrouterCancel: vi.fn(),
  loadKeys: vi.fn(),
  setAiProvider: vi.fn(),
  existing: undefined as { base_url: string } | undefined,
}));

vi.mock("~/ai/providers/llm-auth-client", () => ({
  commands: {
    openrouterSignIn: mocks.openrouterSignIn,
    openrouterCancel: mocks.openrouterCancel,
  },
}));

vi.mock("~/settings/providers", () => ({
  loadSecureAiProviderApiKeys: mocks.loadKeys,
  setAiProvider: mocks.setAiProvider,
  useAiProvider: () => mocks.existing,
}));

function renderCard(setApiKey: (k: string) => void) {
  const client = new QueryClient();
  return render(
    <QueryClientProvider client={client}>
      <OpenRouterSignIn setApiKey={setApiKey} />
    </QueryClientProvider>,
  );
}

describe("OpenRouterSignIn", () => {
  afterEach(() => {
    cleanup();
    vi.clearAllMocks();
    mocks.existing = undefined;
  });

  it("syncs the stored key into the form and creates the provider row with the default base URL", async () => {
    mocks.openrouterSignIn.mockResolvedValue({ status: "ok", data: null });
    mocks.loadKeys.mockResolvedValue({ "llm:openrouter": "sk-or-v1-new" });
    mocks.setAiProvider.mockResolvedValue(undefined);
    const setApiKey = vi.fn();

    renderCard(setApiKey);
    fireEvent.click(screen.getByText("Sign in with OpenRouter"));

    await waitFor(() => expect(setApiKey).toHaveBeenCalledWith("sk-or-v1-new"));
    expect(mocks.setAiProvider).toHaveBeenCalledWith("llm", "openrouter", {
      base_url: "https://openrouter.ai/api/v1",
      api_key: "sk-or-v1-new",
    });
  });

  it("keeps a customised base URL", async () => {
    mocks.existing = { base_url: "https://proxy.example/v1" };
    mocks.openrouterSignIn.mockResolvedValue({ status: "ok", data: null });
    mocks.loadKeys.mockResolvedValue({ "llm:openrouter": "k" });
    mocks.setAiProvider.mockResolvedValue(undefined);

    renderCard(vi.fn());
    fireEvent.click(screen.getByText("Sign in with OpenRouter"));

    await waitFor(() => expect(mocks.setAiProvider).toHaveBeenCalled());
    expect(mocks.setAiProvider.mock.calls[0][2].base_url).toBe(
      "https://proxy.example/v1",
    );
  });

  it("shows the failure and leaves the form alone", async () => {
    mocks.openrouterSignIn.mockResolvedValue({
      status: "error",
      error: "token exchange error: OpenRouter returned 403",
    });
    const setApiKey = vi.fn();

    renderCard(setApiKey);
    fireEvent.click(screen.getByText("Sign in with OpenRouter"));

    expect(await screen.findByText(/returned 403/)).toBeTruthy();
    expect(setApiKey).not.toHaveBeenCalled();
    expect(mocks.setAiProvider).not.toHaveBeenCalled();
  });

  it("cancel stops the flow without surfacing an error", async () => {
    let resolveSignIn: (v: unknown) => void = () => {};
    mocks.openrouterSignIn.mockReturnValue(
      new Promise((resolve) => {
        resolveSignIn = resolve;
      }),
    );

    renderCard(vi.fn());
    fireEvent.click(screen.getByText("Sign in with OpenRouter"));
    fireEvent.click(await screen.findByText("Cancel"));
    expect(mocks.openrouterCancel).toHaveBeenCalled();

    resolveSignIn({ status: "error", error: "sign-in cancelled" });
    await waitFor(() =>
      expect(screen.getByText("Sign in with OpenRouter")).toBeTruthy(),
    );
    expect(screen.queryByText(/cancelled/)).toBeNull();
  });
});

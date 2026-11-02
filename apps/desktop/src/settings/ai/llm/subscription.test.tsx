import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { SubscriptionProvidersSection } from "./configure";
import { SubscriptionDisclosureModal } from "./disclosure-modal";
import { PROVIDERS, SUBSCRIPTION_PROVIDERS } from "./shared";
import { SubscriptionCard } from "./subscription-card";

import type { LlmAuthCatalogProvider } from "~/ai/providers/llm-auth-client";

const mocks = vi.hoisted(() => ({
  openUrl: vi.fn(),
  status: vi.fn(),
  start: vi.fn(),
  complete: vi.fn(),
  cancel: vi.fn(),
  logout: vi.fn(),
  catalog: vi.fn(),
  setSettingValue: vi.fn(),
  configValues: {
    llm_subscription_disclosure_accepted: false,
  },
}));

vi.mock("@hypr/plugin-opener2", () => ({
  commands: {
    openUrl: mocks.openUrl,
  },
}));

vi.mock("~/ai/providers/llm-auth-client", () => ({
  commands: {
    status: mocks.status,
    start: mocks.start,
    complete: mocks.complete,
    cancel: mocks.cancel,
    logout: mocks.logout,
    catalog: mocks.catalog,
  },
  DEFAULT_CATALOG: {
    providers: [
      {
        id: "anthropic",
        enabled: true,
        label: "Anthropic (Claude)",
        beta_headers: {},
        system_prefix: null,
        base_url: "https://api.anthropic.com/v1",
        notes: "Claude OAuth",
      },
      {
        id: "openai_codex",
        enabled: true,
        label: "OpenAI Codex",
        beta_headers: {},
        system_prefix: null,
        base_url: "https://chatgpt.com/backend-api/codex",
        notes: "Codex OAuth",
      },
      {
        id: "gemini",
        enabled: false,
        label: "Google Gemini",
        beta_headers: {},
        system_prefix: null,
        base_url: "https://cloudcode-pa.googleapis.com",
        notes: "Gemini OAuth",
      },
    ],
  },
}));

vi.mock("~/shared/config", () => ({
  useConfigValue: (key: string) => {
    if (key === "llm_subscription_disclosure_accepted") {
      return mocks.configValues.llm_subscription_disclosure_accepted;
    }
    return undefined;
  },
  useConfigValues: () => ({}),
}));

vi.mock("~/settings/queries", () => ({
  useSetSettingValue: (key: string) => (value: unknown) => {
    if (key === "llm_subscription_disclosure_accepted") {
      mocks.configValues.llm_subscription_disclosure_accepted =
        value as boolean;
    }
    mocks.setSettingValue(key, value);
  },
}));

function renderWithQuery(ui: React.ReactElement) {
  const queryClient = new QueryClient({
    defaultOptions: {
      queries: {
        retry: false,
      },
    },
  });
  return render(
    <QueryClientProvider client={queryClient}>{ui}</QueryClientProvider>,
  );
}
afterEach(() => {
  cleanup();
});

describe("Subscription Providers & Catalog Gating", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.configValues.llm_subscription_disclosure_accepted = false;
    mocks.status.mockResolvedValue({
      status: "ok",
      data: { signed_in: false, account_label: null, expires_at: null },
    });
    mocks.catalog.mockResolvedValue({
      status: "ok",
      data: {
        providers: [
          {
            id: "anthropic",
            enabled: true,
            label: "Anthropic (Claude)",
            beta_headers: {},
            system_prefix: null,
            base_url: "https://api.anthropic.com/v1",
            notes: "Claude OAuth",
          },
          {
            id: "openai_codex",
            enabled: true,
            label: "OpenAI Codex",
            beta_headers: {},
            system_prefix: null,
            base_url: "https://chatgpt.com/backend-api/codex",
            notes: "Codex OAuth",
          },
          {
            id: "gemini",
            enabled: false, // disabled in catalog
            label: "Google Gemini",
            beta_headers: {},
            system_prefix: null,
            base_url: "https://cloudcode-pa.googleapis.com",
            notes: "Gemini OAuth",
          },
        ],
      },
    });
  });

  it("includes subscription providers in the main PROVIDERS list", () => {
    const ids = PROVIDERS.map((p) => p.id);
    expect(ids).toContain("anthropic_subscription");
    expect(ids).toContain("openai_subscription");
    expect(ids).toContain("gemini_subscription");
  });

  it("gating: hides providers where enabled is false in catalog", async () => {
    renderWithQuery(<SubscriptionProvidersSection />);

    // Open the collapsed section
    const trigger = await screen.findByText("Use my subscription (unofficial)");
    fireEvent.click(trigger);

    await waitFor(() => {
      expect(screen.getByText("Anthropic (Claude)")).toBeTruthy();
      expect(screen.getByText("OpenAI Codex")).toBeTruthy();
      expect(screen.queryByText("Google Gemini")).toBeNull();
    });
  });

  it("gating: hides entire section if all providers are disabled", async () => {
    mocks.catalog.mockResolvedValueOnce({
      status: "ok",
      data: {
        providers: [
          {
            id: "anthropic",
            enabled: false,
            label: "Anthropic",
            beta_headers: {},
            system_prefix: null,
            base_url: "",
            notes: "",
          },
          {
            id: "openai_codex",
            enabled: false,
            label: "OpenAI",
            beta_headers: {},
            system_prefix: null,
            base_url: "",
            notes: "",
          },
          {
            id: "gemini",
            enabled: false,
            label: "Gemini",
            beta_headers: {},
            system_prefix: null,
            base_url: "",
            notes: "",
          },
        ],
      },
    });

    const { container } = renderWithQuery(<SubscriptionProvidersSection />);

    await waitFor(() => {
      expect(screen.queryByText("Use my subscription (unofficial)")).toBeNull();
      expect(container.firstChild).toBeNull();
    });
  });
});

describe("Disclosure Modal & Persistence", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.configValues.llm_subscription_disclosure_accepted = false;
  });

  it("persists acceptance in app settings and triggers onAccept", () => {
    const onOpenChange = vi.fn();
    const onAccept = vi.fn();

    render(
      <SubscriptionDisclosureModal
        open={true}
        onOpenChange={onOpenChange}
        onAccept={onAccept}
      />,
    );

    expect(
      screen.getByText(/Use Subscription Credentials \(Unofficial\)/),
    ).toBeTruthy();
    expect(screen.getByText(/Not endorsed/)).toBeTruthy();
    expect(screen.getByText(/Account risk/)).toBeTruthy();
    expect(screen.getByText(/Privacy/)).toBeTruthy();

    const acceptBtn = screen.getByRole("button", {
      name: /I Understand and Accept/i,
    });
    fireEvent.click(acceptBtn);

    expect(mocks.setSettingValue).toHaveBeenCalledWith(
      "llm_subscription_disclosure_accepted",
      true,
    );
    expect(mocks.configValues.llm_subscription_disclosure_accepted).toBe(true);
    expect(onOpenChange).toHaveBeenCalledWith(false);
    expect(onAccept).toHaveBeenCalled();
  });
});

describe("SubscriptionCard sign-in workflow", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.configValues.llm_subscription_disclosure_accepted = false;
    mocks.status.mockResolvedValue({
      status: "ok",
      data: { signed_in: false, account_label: null, expires_at: null },
    });
    mocks.start.mockResolvedValue({
      status: "ok",
      data: {
        authorize_url: "https://claude.ai/oauth/authorize?foo=bar",
        mode: "loopback",
      },
    });
    mocks.complete.mockResolvedValue({
      status: "ok",
      data: {
        signed_in: true,
        account_label: "user@example.com",
        expires_at: null,
      },
    });
  });

  const anthropicProvider = SUBSCRIPTION_PROVIDERS.find(
    (p) => p.id === "anthropic_subscription",
  )!;
  const anthropicCatalog: LlmAuthCatalogProvider = {
    id: "anthropic",
    enabled: true,
    label: "Anthropic (Claude)",
    beta_headers: {},
    system_prefix: null,
    base_url: "https://api.anthropic.com/v1",
    notes: "Claude OAuth",
  };

  it("opens disclosure modal on first click if not yet accepted", async () => {
    renderWithQuery(
      <SubscriptionCard
        provider={anthropicProvider}
        catalogProvider={anthropicCatalog}
      />,
    );

    const signInBtn = await screen.findByRole("button", { name: /Sign in/i });
    fireEvent.click(signInBtn);

    // Modal should be open
    expect(
      screen.getByText(/Use Subscription Credentials \(Unofficial\)/),
    ).toBeTruthy();
    expect(mocks.start).not.toHaveBeenCalled();

    // Accept modal
    const acceptBtn = screen.getByRole("button", {
      name: /I Understand and Accept/i,
    });
    fireEvent.click(acceptBtn);

    // Starts flow and opens authorize_url
    await waitFor(() => {
      expect(mocks.start).toHaveBeenCalledWith("anthropic");
      expect(mocks.openUrl).toHaveBeenCalledWith(
        "https://claude.ai/oauth/authorize?foo=bar",
        null,
      );
      expect(mocks.complete).toHaveBeenCalledWith("anthropic", null);
    });
  });

  it("displays paste input when mode is paste", async () => {
    mocks.configValues.llm_subscription_disclosure_accepted = true;
    mocks.start.mockResolvedValueOnce({
      status: "ok",
      data: {
        authorize_url: "https://platform.claude.com/oauth",
        mode: "paste",
      },
    });

    renderWithQuery(
      <SubscriptionCard
        provider={anthropicProvider}
        catalogProvider={anthropicCatalog}
      />,
    );

    const signInBtn = await screen.findByRole("button", { name: /Sign in/i });
    fireEvent.click(signInBtn);

    await waitFor(() => {
      expect(
        screen.getByPlaceholderText(/Paste authorization code here/i),
      ).toBeTruthy();
    });

    const input = screen.getByPlaceholderText(/Paste authorization code here/i);
    fireEvent.change(input, { target: { value: "test-auth-code-123" } });

    const completeBtn = screen.getByRole("button", { name: /Complete/i });
    fireEvent.click(completeBtn);

    await waitFor(() => {
      expect(mocks.complete).toHaveBeenCalledWith(
        "anthropic",
        "test-auth-code-123",
      );
    });
  });

  it("supports cancelling sign in", async () => {
    mocks.configValues.llm_subscription_disclosure_accepted = true;
    // Keep complete unresolved
    mocks.complete.mockImplementationOnce(() => new Promise(() => {}));

    renderWithQuery(
      <SubscriptionCard
        provider={anthropicProvider}
        catalogProvider={anthropicCatalog}
      />,
    );

    const signInBtn = await screen.findByRole("button", { name: /Sign in/i });
    fireEvent.click(signInBtn);

    const cancelBtn = await screen.findByRole("button", { name: /Cancel/i });
    fireEvent.click(cancelBtn);

    expect(mocks.cancel).toHaveBeenCalledWith("anthropic");
  });

  it("displays signed in status chip and supports signing out", async () => {
    mocks.status.mockResolvedValue({
      status: "ok",
      data: {
        signed_in: true,
        account_label: "skywalker@jediacademy.edu",
        expires_at: new Date(Date.now() + 3600 * 1000).toISOString(),
      },
    });

    renderWithQuery(
      <SubscriptionCard
        provider={anthropicProvider}
        catalogProvider={anthropicCatalog}
      />,
    );

    await waitFor(() => {
      expect(screen.getByText("skywalker@jediacademy.edu")).toBeTruthy();
      expect(screen.getByRole("button", { name: /Sign out/i })).toBeTruthy();
    });

    const signOutBtn = screen.getByRole("button", { name: /Sign out/i });
    fireEvent.click(signOutBtn);

    expect(mocks.logout).toHaveBeenCalledWith("anthropic");
  });
});

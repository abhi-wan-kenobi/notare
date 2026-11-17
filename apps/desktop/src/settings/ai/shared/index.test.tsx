import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { Accordion } from "@hypr/ui/components/ui/accordion";

import { NonHyprProviderCard } from "./index";

const mocks = vi.hoisted(() => ({
  save: vi.fn(),
}));

vi.mock("~/auth/billing", () => ({
  useBillingAccess: () => ({ isPaid: false }),
}));

vi.mock("~/settings/providers", () => ({
  useAiProvider: () => undefined,
  useAiProviders: () => ({}),
  useSetAiProvider: () => mocks.save,
}));

vi.mock("./hypr-cloud-button", () => ({}));
vi.mock("./model-combobox", () => ({}));

const provider = {
  id: "openai",
  displayName: "OpenAI",
  icon: null,
  baseUrl: "https://api.openai.com/v1",
  requirements: [
    { kind: "requires_config" as const, fields: ["api_key" as const] },
  ],
};

function renderCard() {
  return render(
    <Accordion type="single" value={provider.id}>
      <NonHyprProviderCard
        config={provider}
        providerType="llm"
        providers={[provider]}
      />
    </Accordion>,
  );
}

function enterApiKey(value: string) {
  fireEvent.change(screen.getByPlaceholderText("Stored in system keychain"), {
    target: { value },
  });
}

function clickSave() {
  fireEvent.click(screen.getByRole("button", { name: /Save$/ }));
}

describe("NonHyprProviderCard save", () => {
  afterEach(() => {
    cleanup();
    vi.clearAllMocks();
    vi.restoreAllMocks();
  });

  it("reports the save only after the provider was stored", async () => {
    mocks.save.mockResolvedValue(undefined);
    renderCard();

    enterApiKey("sk-test");
    clickSave();

    await waitFor(() => expect(screen.getByText("Saved")).toBeTruthy());
    expect(mocks.save).toHaveBeenCalledWith({
      type: "llm",
      base_url: "https://api.openai.com/v1",
      api_key: "sk-test",
    });
  });

  it("does not report a save when the form is invalid", async () => {
    renderCard();

    clickSave();

    // Once inline under the field, once in the save status next to the button.
    await waitFor(() =>
      expect(
        screen.getAllByText("API key is required for HTTPS URLs"),
      ).toHaveLength(2),
    );
    expect(mocks.save).not.toHaveBeenCalled();
    expect(screen.queryByText(/Saved/)).toBeNull();
  });

  it("shows the failure instead of a save when storing the provider fails", async () => {
    vi.spyOn(console, "error").mockImplementation(() => {});
    mocks.save.mockRejectedValue(new Error("keychain is locked"));
    renderCard();

    enterApiKey("sk-test");
    clickSave();

    await waitFor(() =>
      expect(screen.getByText("keychain is locked")).toBeTruthy(),
    );
    expect(screen.queryByText(/Saved/)).toBeNull();
  });
});

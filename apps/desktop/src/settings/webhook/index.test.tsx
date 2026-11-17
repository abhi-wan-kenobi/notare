import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  getSettings: vi.fn(),
  setSettings: vi.fn(),
  setSecret: vi.fn(),
  clearSecret: vi.fn(),
  recentDeliveries: vi.fn(),
  testWebhook: vi.fn(),
}));

vi.mock("@hypr/plugin-webhook", () => ({
  commands: {
    getSettings: mocks.getSettings,
    setSettings: mocks.setSettings,
    setSecret: mocks.setSecret,
    clearSecret: mocks.clearSecret,
    recentDeliveries: mocks.recentDeliveries,
    testWebhook: mocks.testWebhook,
  },
}));

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

import { SettingsWebhook } from "./index";

const STORED = {
  endpoint_url: "https://example.com/hook",
  enabled: true,
  events: { action_items_updated: false, session_enhanced: false },
  has_secret: true,
};

function renderPage() {
  const queryClient = new QueryClient({
    defaultOptions: {
      queries: { retry: false },
      mutations: { retry: false },
    },
  });

  return render(
    <QueryClientProvider client={queryClient}>
      <SettingsWebhook />
    </QueryClientProvider>,
  );
}

describe("SettingsWebhook", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.getSettings.mockResolvedValue({ status: "ok", data: STORED });
    mocks.recentDeliveries.mockResolvedValue({ status: "ok", data: [] });
  });

  afterEach(() => {
    cleanup();
  });

  it("shows a failed settings read inline instead of throwing out of the section", async () => {
    mocks.getSettings.mockResolvedValue({
      status: "error",
      error: "store is corrupt",
    });

    renderPage();

    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain("store is corrupt");
    expect(screen.getByText("Webhooks")).toBeTruthy();
  });

  it("puts a switch and the endpoint back and says why when saving fails", async () => {
    mocks.setSettings.mockResolvedValue({
      status: "error",
      error: "disk full",
    });

    renderPage();

    const [master] = await screen.findAllByRole("switch");
    expect(master.getAttribute("aria-checked")).toBe("true");
    fireEvent.click(master);

    await waitFor(() =>
      expect(screen.getByRole("alert").textContent).toContain("disk full"),
    );
    expect(master.getAttribute("aria-checked")).toBe("true");

    const endpoint = screen.getByPlaceholderText(
      "https://example.com/notare/webhook",
    ) as HTMLInputElement;
    fireEvent.change(endpoint, { target: { value: "https://other.test" } });
    fireEvent.blur(endpoint);

    await waitFor(() => expect(mocks.setSettings).toHaveBeenCalledTimes(2));
    await waitFor(() => expect(endpoint.value).toBe(STORED.endpoint_url));
  });

  it("says why the signing secret could not be saved", async () => {
    mocks.setSecret.mockResolvedValue({
      status: "error",
      error: "keychain is locked",
    });

    renderPage();

    fireEvent.change(await screen.findByPlaceholderText("••••••••"), {
      target: { value: "s3cret" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Rotate" }));

    await waitFor(() =>
      expect(screen.getByRole("alert").textContent).toContain(
        "keychain is locked",
      ),
    );
  });

  it("says why the signing secret could not be cleared", async () => {
    mocks.clearSecret.mockResolvedValue({
      status: "error",
      error: "keychain is locked",
    });

    renderPage();

    fireEvent.click(await screen.findByRole("button", { name: "Clear" }));

    await waitFor(() =>
      expect(screen.getByRole("alert").textContent).toContain(
        "keychain is locked",
      ),
    );
    expect(screen.getByRole("button", { name: "Clear" })).toBeTruthy();
  });

  it("says why the test event could not be sent", async () => {
    mocks.testWebhook.mockResolvedValue({
      status: "error",
      error: "no signing secret",
    });

    renderPage();

    fireEvent.click(
      await screen.findByRole("button", { name: "Send test event" }),
    );

    await waitFor(() =>
      expect(screen.getByRole("alert").textContent).toContain(
        "no signing secret",
      ),
    );
  });
});

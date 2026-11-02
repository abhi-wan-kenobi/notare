import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const calendarCommands = vi.hoisted(() => ({
  gmailStatus: vi.fn(),
  gmailConnect: vi.fn(),
  gmailDisconnect: vi.fn(),
}));

vi.mock("@hypr/plugin-calendar", () => ({
  commands: calendarCommands,
}));

vi.mock("@lingui/react/macro", () => ({
  Trans: ({ children }: { children: React.ReactNode }) => <>{children}</>,
  useLingui: () => ({
    t: (str: string | TemplateStringsArray) =>
      typeof str === "string" ? str : str[0],
  }),
}));

import { SettingsGmail } from "./index";

function renderWithClient(ui: React.ReactElement) {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return render(
    <QueryClientProvider client={queryClient}>{ui}</QueryClientProvider>,
  );
}

describe("SettingsGmail", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("shows unconfigured client banner when has_client is false", async () => {
    calendarCommands.gmailStatus.mockResolvedValue({
      status: "ok",
      data: {
        has_client: false,
        connected: false,
        accounts: [],
      },
    });

    renderWithClient(<SettingsGmail />);

    await waitFor(() => {
      expect(
        screen.getByText(/Gmail OAuth client not configured in this build/),
      ).toBeDefined();
    });
  });

  it("renders connected accounts and connects new account when has_client is true", async () => {
    calendarCommands.gmailStatus.mockResolvedValue({
      status: "ok",
      data: {
        has_client: true,
        connected: true,
        accounts: ["alice@example.com"],
      },
    });

    renderWithClient(<SettingsGmail />);

    await waitFor(() => {
      expect(screen.getByText("alice@example.com")).toBeDefined();
      expect(screen.getByText("Disconnect")).toBeDefined();
    });
  });

  it("calls disconnect when disconnect button is clicked", async () => {
    calendarCommands.gmailStatus.mockResolvedValue({
      status: "ok",
      data: {
        has_client: true,
        connected: true,
        accounts: ["alice@example.com"],
      },
    });
    calendarCommands.gmailDisconnect.mockResolvedValue({
      status: "ok",
      data: {
        has_client: true,
        connected: false,
        accounts: [],
      },
    });

    renderWithClient(<SettingsGmail />);

    await waitFor(() => {
      expect(screen.getByText("Disconnect")).toBeDefined();
    });

    fireEvent.click(screen.getByText("Disconnect"));
    await waitFor(() => {
      expect(calendarCommands.gmailDisconnect).toHaveBeenCalledWith(
        "alice@example.com",
      );
    });
  });
});

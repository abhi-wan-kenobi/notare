import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const calendarCommands = vi.hoisted(() => ({
  outlookAccountStatus: vi.fn(),
  outlookSetClientId: vi.fn(),
  outlookConnect: vi.fn(),
  outlookDisconnect: vi.fn(),
  outlookReset: vi.fn(),
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

vi.mock("../context", () => ({
  useSync: () => ({
    scheduleSync: vi.fn(),
  }),
}));

vi.mock("../oauth/calendar-selection", () => ({
  OAuthCalendarSelection: () => <div data-testid="oauth-calendar-selection" />,
  useOAuthCalendarSelection: () => ({
    groups: [],
    handleRefresh: vi.fn(),
    handleToggle: vi.fn(),
    isLoading: false,
  }),
}));

import type { CalendarProvider } from "../shared";
import { OutlookDirectContent } from "./content";

const mockConfig: CalendarProvider = {
  disabled: false,
  id: "outlook",
  displayName: "Outlook",
  icon: null,
};

function renderWithClient(ui: React.ReactElement) {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return render(
    <QueryClientProvider client={queryClient}>{ui}</QueryClientProvider>,
  );
}

describe("OutlookDirectContent", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("renders bundled client state with Sign in with Microsoft and Advanced option", async () => {
    calendarCommands.outlookAccountStatus.mockResolvedValue({
      status: "ok",
      data: {
        has_client: false,
        has_bundled_client: true,
        connected: false,
        client_id: null,
        accounts: [],
      },
    });

    renderWithClient(<OutlookDirectContent config={mockConfig} />);

    await waitFor(() => {
      expect(screen.getByText("Sign in with Microsoft")).toBeDefined();
      expect(
        screen.getByText("Advanced: use your own Application (client) ID"),
      ).toBeDefined();
    });
  });

  it("reveals BYO client input when advanced is toggled and saves client id", async () => {
    calendarCommands.outlookAccountStatus.mockResolvedValue({
      status: "ok",
      data: {
        has_client: false,
        has_bundled_client: true,
        connected: false,
        client_id: null,
        accounts: [],
      },
    });
    calendarCommands.outlookSetClientId.mockResolvedValue({
      status: "ok",
      data: { client_id: "custom-client-id-123" },
    });

    renderWithClient(<OutlookDirectContent config={mockConfig} />);

    await waitFor(() => {
      expect(
        screen.getByText("Advanced: use your own Application (client) ID"),
      ).toBeDefined();
    });

    fireEvent.click(
      screen.getByText("Advanced: use your own Application (client) ID"),
    );

    const input = screen.getByPlaceholderText(
      "00000000-0000-0000-0000-000000000000",
    );
    fireEvent.change(input, { target: { value: "custom-client-id-123" } });

    const saveButton = screen.getByText("Save");
    fireEvent.click(saveButton);

    await waitFor(() => {
      expect(calendarCommands.outlookSetClientId).toHaveBeenCalledWith(
        "custom-client-id-123",
      );
    });
  });

  it("renders connected accounts and handles disconnect", async () => {
    calendarCommands.outlookAccountStatus.mockResolvedValue({
      status: "ok",
      data: {
        has_client: true,
        has_bundled_client: true,
        connected: true,
        client_id: "ms-app-id",
        accounts: [
          { email: "user@contoso.com", granted_scopes: ["Calendars.Read"] },
        ],
      },
    });
    calendarCommands.outlookDisconnect.mockResolvedValue({
      status: "ok",
      data: {
        has_client: true,
        has_bundled_client: true,
        connected: false,
        client_id: "ms-app-id",
        accounts: [],
      },
    });

    renderWithClient(<OutlookDirectContent config={mockConfig} />);

    await waitFor(() => {
      expect(screen.getByText("user@contoso.com")).toBeDefined();
      expect(screen.getByText("Remove")).toBeDefined();
      expect(screen.getByText("+ Add Microsoft account")).toBeDefined();
    });

    fireEvent.click(screen.getByText("Remove"));

    await waitFor(() => {
      expect(calendarCommands.outlookDisconnect).toHaveBeenCalledWith(
        "user@contoso.com",
      );
    });
  });
});

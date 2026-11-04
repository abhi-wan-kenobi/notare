import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  hidden: [] as string[],
  setCalendarProviderHidden: vi.fn(async () => undefined),
}));

vi.mock("@lingui/react/macro", () => ({
  Trans: ({ children }: { children: React.ReactNode }) => <>{children}</>,
  useLingui: () => ({
    t: (strings: TemplateStringsArray | string, ...values: unknown[]) =>
      typeof strings === "string"
        ? strings
        : strings.reduce((out, part, i) => out + part + (values[i] ?? ""), ""),
  }),
}));

vi.mock("@tauri-apps/plugin-os", () => ({ platform: () => "linux" }));

vi.mock("~/auth", () => ({ useAuth: () => ({ session: null }) }));
vi.mock("~/auth/billing", () => ({
  useBillingAccess: () => ({
    isPaid: false,
    isPro: false,
    upgradeToPro: vi.fn(),
  }),
}));
vi.mock("~/auth/useConnections", () => ({
  useConnections: () => ({ data: undefined, isPending: false, isError: false }),
}));
vi.mock("~/shared/hooks/usePermissions", () => ({
  usePermission: () => ({ status: "authorized" }),
}));
vi.mock("~/shared/hooks/useNativeContextMenu", () => ({
  useNativeContextMenu: () => undefined,
}));
vi.mock("~/shared/integration", () => ({ openIntegrationUrl: vi.fn() }));

vi.mock("~/calendar/hidden-providers", () => ({
  useHiddenCalendarProviders: () => mocks.hidden,
  setCalendarProviderHidden: mocks.setCalendarProviderHidden,
}));

vi.mock("./apple/calendar-selection", () => ({
  AppleCalendarSelection: () => null,
}));
vi.mock("./apple/permission", () => ({
  AccessPermissionRow: () => null,
  TroubleShootingLink: () => null,
}));
vi.mock("./google/content", () => ({ GoogleDirectContent: () => null }));
vi.mock("./outlook/content", () => ({ OutlookDirectContent: () => null }));
vi.mock("./ics/content", () => ({ IcsContent: () => null }));
vi.mock("./oauth/provider-content", () => ({
  OAuthProviderContent: () => null,
}));

import { CalendarSidebarContent } from "./sidebar";

describe("CalendarSidebarContent hidden providers", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.hidden = [];
  });

  afterEach(cleanup);

  it("shows every provider and no hidden row when nothing is hidden", () => {
    render(<CalendarSidebarContent />);

    expect(screen.getByText("Google")).toBeTruthy();
    expect(screen.getByText("Outlook")).toBeTruthy();
    expect(screen.getByText("Calendar files (.ics)")).toBeTruthy();
    expect(screen.queryByTestId("hidden-calendar-providers")).toBeNull();
  });

  it("collapses a hidden provider into a single row with a Show action", () => {
    mocks.hidden = ["ics"];
    render(<CalendarSidebarContent />);

    expect(screen.getByText("Google")).toBeTruthy();
    expect(screen.queryByLabelText("Hide Calendar files (.ics)")).toBeNull();
    const row = screen.getByTestId("hidden-calendar-providers");
    expect(row.textContent).toContain(
      "Hidden providers: Calendar files (.ics)",
    );

    fireEvent.click(screen.getByRole("button", { name: "Show" }));
    expect(mocks.setCalendarProviderHidden).toHaveBeenCalledTimes(1);
    expect(mocks.setCalendarProviderHidden).toHaveBeenCalledWith("ics", false);
  });

  it("lists several hidden providers and restores them all", () => {
    mocks.hidden = ["google", "ics"];
    render(<CalendarSidebarContent />);

    expect(
      screen.getByTestId("hidden-calendar-providers").textContent,
    ).toContain("Hidden providers: Google, Calendar files (.ics)");
    expect(screen.getByText("Outlook")).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: "Show all" }));
    expect(mocks.setCalendarProviderHidden.mock.calls).toEqual([
      ["google", false],
      ["ics", false],
    ]);
  });

  it("hides a provider from its header action", () => {
    render(<CalendarSidebarContent />);

    fireEvent.click(screen.getByLabelText("Hide Outlook"));

    expect(mocks.setCalendarProviderHidden).toHaveBeenCalledWith(
      "outlook",
      true,
    );
  });
});

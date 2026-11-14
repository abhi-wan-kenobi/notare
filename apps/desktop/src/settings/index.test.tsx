import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  connect: vi.fn(async () => ({ status: "ok", data: null })),
  scheduleSync: vi.fn(() => "calendar-sync-run"),
}));

vi.mock("@tauri-apps/plugin-os", () => ({ platform: () => "linux" }));
vi.mock("tinytick/ui-react", () => ({
  useScheduleTaskRunCallback: () => mocks.scheduleSync,
  useTaskRunRunning: () => false,
}));
vi.mock("~/services/calendar", () => ({
  CALENDAR_SYNC_TASK_ID: "calendar-sync",
  syncCalendarEventsForRange: vi.fn(),
}));
vi.mock("@hypr/plugin-calendar", () => ({
  commands: {
    googleAccountStatus: async () => ({
      status: "ok",
      data: { has_client: false, has_bundled_client: true, connected: false },
    }),
    googleConnect: mocks.connect,
  },
}));
vi.mock("@hypr/plugin-opener2", () => ({ commands: {} }));
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn() }));
vi.mock("~/calendar/hidden-providers", () => ({
  setCalendarProviderHidden: vi.fn(),
  useHiddenCalendarProviders: () => [],
}));
vi.mock("~/calendar/components/oauth/calendar-selection", () => ({
  OAuthCalendarSelection: () => null,
  useOAuthCalendarSelection: () => ({}),
}));
import { GoogleDirectContent } from "~/calendar/components/google/content";
import { PROVIDERS } from "~/calendar/components/shared";

// Keep the failing consumer and its SyncProvider real. Limit the provider list
// to Google so the test does not need unrelated native calendar integrations.
vi.mock("~/calendar/components/sidebar", () => ({
  CalendarSidebarContent: () => (
    <GoogleDirectContent config={PROVIDERS.find((p) => p.id === "google")!} />
  ),
}));
vi.mock("~/store/zustand/tabs", () => ({
  useTabs: (selector: (state: unknown) => unknown) =>
    selector({ updateSettingsTabState: vi.fn() }),
}));
vi.mock("~/shared/main", () => ({
  StandardContentWrapper: ({ children }: { children: React.ReactNode }) =>
    children,
}));
vi.mock("~/settings/account", () => ({ AccountSettingsView: () => null }));
vi.mock("~/settings/ai/llm", () => ({ LLM: () => null }));
vi.mock("~/settings/ai/stt", () => ({ STT: () => null }));
vi.mock("~/settings/dictation", () => ({ SettingsDictation: () => null }));
vi.mock("~/settings/general", () => ({
  SettingsApp: () => null,
  SettingsNotifications: () => null,
  SettingsPermissions: () => null,
}));
vi.mock("~/settings/gmail", () => ({ SettingsGmail: () => null }));
vi.mock("~/settings/mcp", () => ({ SettingsMcp: () => null }));
vi.mock("~/settings/personalization", () => ({
  SettingsPersonalization: () => null,
}));
vi.mock("~/settings/recipes", () => ({ SettingsRecipes: () => null }));
vi.mock("~/settings/sync", () => ({ SettingsSync: () => null }));
vi.mock("~/settings/webhook", () => ({ SettingsWebhook: () => null }));

import { TabContentSettings } from "./index";

import { ContentErrorBoundary } from "~/shared/content-error-boundary";

const clients: QueryClient[] = [];
afterEach(() => {
  cleanup();
  clients.splice(0).forEach((client) => client.clear());
  vi.clearAllMocks();
});

describe("Integrations & Sync settings", () => {
  it("offers Google sign-in without falling into the section error boundary", async () => {
    const client = new QueryClient({
      defaultOptions: { queries: { retry: false } },
    });
    clients.push(client);
    render(
      <QueryClientProvider client={client}>
        <ContentErrorBoundary>
          <TabContentSettings
            tab={{
              type: "settings",
              active: true,
              slotId: "settings",
              pinned: false,
              state: { tab: "integrations" },
            }}
          />
        </ContentErrorBoundary>
      </QueryClientProvider>,
    );

    const signIn = await screen.findByRole("button", {
      name: "Sign in with Google",
    });
    expect(screen.queryByText("This section hit a problem")).toBeNull();
    fireEvent.click(signIn);
    await waitFor(() => expect(mocks.connect).toHaveBeenCalledOnce());
    await waitFor(() => expect(mocks.scheduleSync).toHaveBeenCalledOnce());
  });
});

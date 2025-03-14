import { createFileRoute, Outlet } from "@tanstack/react-router";

import {
  LeftSidebarProvider,
  NewNoteProvider,
  OngoingSessionProvider,
  RightPanelProvider,
  SearchProvider,
  SessionsProvider,
  SettingsPanelProvider,
} from "@/contexts";

export const Route = createFileRoute("/app/note")({
  component: Component,
  loader: async ({ context: { sessionsStore } }) => {
    return sessionsStore;
  },
});

function Component() {
  const store = Route.useLoaderData();

  return (
    <SessionsProvider store={store}>
      <OngoingSessionProvider>
        <LeftSidebarProvider>
          <RightPanelProvider>
            <SearchProvider>
              <SettingsPanelProvider>
                <NewNoteProvider>
                  <Outlet />
                </NewNoteProvider>
              </SettingsPanelProvider>
            </SearchProvider>
          </RightPanelProvider>
        </LeftSidebarProvider>
      </OngoingSessionProvider>
    </SessionsProvider>
  );
}

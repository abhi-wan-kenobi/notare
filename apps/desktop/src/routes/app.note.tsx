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
  component: RouteComponent,
});

function RouteComponent() {
  return (
    <SessionsProvider>
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

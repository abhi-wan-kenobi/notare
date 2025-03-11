import { useEffect } from "react";
import { createFileRoute } from "@tanstack/react-router";
import { Outlet } from "@tanstack/react-router";

import { HyprProvider } from "@/contexts/hypr";
import { LeftSidebarProvider } from "@/contexts/left-sidebar";
import { NewNoteProvider } from "@/contexts/new-note";
import { OngoingSessionProvider } from "@/contexts/ongoing-session";
import { RightPanelProvider } from "@/contexts/right-panel";
import { SearchProvider } from "@/contexts/search-palette";
import { SettingsPanelProvider } from "@/contexts/settings-panel";

import { registerTemplates } from "@/templates";
import RightPanel from "@/components/note/right-panel";
import LeftSidebar from "@/components/left-sidebar";
import Toolbar from "@/components/toolbar";

export const Route = createFileRoute("/app")({
  component: Component,
});

function Component() {
  useEffect(() => {
    registerTemplates();
    initExtensions();
  }, []);

  return (
    <HyprProvider>
      <OngoingSessionProvider>
        <SessionProvider>
          <LeftSidebarProvider>
            <RightPanelProvider>
              <SearchProvider>
                <SettingsPanelProvider>
                  <NewNoteProvider>
                    <div className="flex h-screen w-screen overflow-hidden">
                      <LeftSidebar />
                      <div className="flex-1 flex h-screen w-screen flex-col overflow-hidden">
                        <Toolbar />
                        <div className="flex h-full overflow-hidden">
                          <div className="flex-1">
                            <Outlet />
                          </div>
                          <RightPanel />
                        </div>
                      </div>
                    </div>
                  </NewNoteProvider>
                </SettingsPanelProvider>
              </SearchProvider>
            </RightPanelProvider>
          </LeftSidebarProvider>
        </SessionProvider>
      </OngoingSessionProvider>
    </HyprProvider>
  );
}

import DinoGameExtension from "@hypr/extension-dino-game";
import SummaryExtension from "@hypr/extension-summary";
import TranscriptExtension from "@hypr/extension-transcript";
import { SessionProvider } from "@/contexts";

function initExtensions() {
  [
    ...Object.values(SummaryExtension),
    ...Object.values(TranscriptExtension),
    ...Object.values(DinoGameExtension),
  ].forEach((group) => {
    group.items.forEach((item) => {
      item.init();
    });
  });
}

import { createFileRoute } from "@tanstack/react-router";
import { Outlet } from "@tanstack/react-router";
import Toolbar from "@/components/toolbar";
import { RightPanelProvider } from "@/contexts/right-panel";

export const Route = createFileRoute("/_nav")({
  component: Component,
});

function Component() {
  return (
    <RightPanelProvider>
      <div className="flex h-screen w-screen flex-col overflow-x-hidden">
        <Toolbar />
        <Outlet />
      </div>
    </RightPanelProvider>
  );
}

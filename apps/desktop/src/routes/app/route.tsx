import {
  createFileRoute,
  Outlet,
  redirect,
  useRouterState,
} from "@tanstack/react-router";

import { TooltipProvider } from "@hypr/ui/components/ui/tooltip";

import {
  getOnboardingNeeded,
  isShellEntryPath,
  normalizeAppPath,
  resolveShellEntryPath,
} from "./-resolve-entry-path";

import { useDeeplinkHandler } from "~/shared/hooks/useDeeplinkHandler";
import { ContentErrorBoundary } from "~/shared/content-error-boundary";
import { ListenerProvider } from "~/stt/contexts";

export const Route = createFileRoute("/app")({
  beforeLoad: async ({ location }) => {
    const pathname = normalizeAppPath(location.pathname);
    const onboardingNeeded = await getOnboardingNeeded();

    if (pathname === "/app/onboarding") {
      if (!onboardingNeeded) {
        throw redirect({ to: await resolveShellEntryPath() });
      }
      return;
    }

    if (onboardingNeeded && isShellEntryPath(pathname)) {
      throw redirect({ to: "/app/onboarding" });
    }
  },
  component: Component,
  loader: async ({ context: { listenerStore } }) => {
    return { listenerStore: listenerStore! };
  },
});

function Component() {
  const { listenerStore } = Route.useLoaderData();
  // A render error inside `<Outlet />` covers every "/app/*" window surface
  // (main w/ tabs, dictation orb, floating meeting bar, composer,
  // instruction, onboarding, a popped-out note) - `RootErrorBoundary` in
  // main.tsx is the only other boundary in the app, and it sits above the
  // router entirely, so without this a crash here would unmount the whole
  // window down to a bare "Reload Notare" screen. Keying the reset off the
  // top-level path clears a caught error the moment the route actually
  // changes, even for a surface that isn't otherwise remounted.
  const pathname = useRouterState({ select: (state) => state.location.pathname });

  useDeeplinkHandler();

  return (
    <TooltipProvider>
      <ListenerProvider store={listenerStore}>
        <ContentErrorBoundary resetKey={pathname}>
          <Outlet />
        </ContentErrorBoundary>
      </ListenerProvider>
    </TooltipProvider>
  );
}

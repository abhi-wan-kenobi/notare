import { lazy, Suspense, useEffect } from "react";
import {
  Outlet,
  createRootRouteWithContext,
  CatchNotFound,
} from "@tanstack/react-router";

import { CatchNotFoundFallback, NotFoundComponent } from "@/components/control";
import { checkForAppUpdates } from "@/utils/updater";
import type { Context } from "@/types";

export const Route = createRootRouteWithContext<Context>()({
  component: Component,
  notFoundComponent: NotFoundComponent,
});

const POSITION = "bottom-right";

function Component() {
  useEffect(() => {
    checkForAppUpdates();
  }, []);

  return (
    <>
      <CatchNotFound fallback={(e) => <CatchNotFoundFallback error={e} />}>
        <Outlet />
      </CatchNotFound>
      <Suspense>
        <TanStackRouterDevtools position={POSITION} initialIsOpen={false} />
        <TanStackQueryDevtools
          buttonPosition={POSITION}
          position="bottom"
          initialIsOpen={false}
        />
      </Suspense>
    </>
  );
}

const TanStackRouterDevtools =
  process.env.NODE_ENV === "production"
    ? () => null
    : lazy(() =>
        import("@tanstack/router-devtools").then((res) => ({
          default: (
            props: React.ComponentProps<typeof res.TanStackRouterDevtools>,
          ) => <res.TanStackRouterDevtools {...props} />,
        })),
      );

const TanStackQueryDevtools =
  process.env.NODE_ENV === "production"
    ? () => null
    : lazy(() =>
        import("@tanstack/react-query-devtools").then((res) => ({
          default: (
            props: React.ComponentProps<typeof res.ReactQueryDevtools>,
          ) => <res.ReactQueryDevtools {...props} />,
        })),
      );

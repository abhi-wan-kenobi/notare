import "@hypr/ui/globals.css";
import "./styles/globals.css";

import { ClerkProvider } from "@clerk/clerk-react";
import { ThemeProvider } from "@hypr/ui/contexts/theme";
import * as Sentry from "@sentry/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { createRouter, RouterProvider } from "@tanstack/react-router";
import ReactDOM from "react-dom/client";
import { routeTree } from "./routeTree.gen";

const router = createRouter({
  routeTree,
  defaultPreload: "intent",
});

declare module "@tanstack/react-router" {
  interface Register {
    router: typeof router;
  }
}

if (!import.meta.env.VITE_CLERK_PUBLISHABLE_KEY) {
  throw new Error("'VITE_CLERK_PUBLISHABLE_KEY' is not set");
}

if (!import.meta.env.VITE_SENTRY_DSN) {
  throw new Error("'VITE_SENTRY_DSN' is not set");
}

Sentry.init({
  dsn: import.meta.env.VITE_SENTRY_DSN,
  integrations: [Sentry.tanstackRouterBrowserTracingIntegration(router)],
  tracesSampleRate: 1.0,
});

const queryClient = new QueryClient();

const enableMocking = async () => {
  const { worker } = await import("./mocks/browser");
  return worker.start();
};

enableMocking().then(() => {
  const rootElement = document.getElementById("app")!;

  if (!rootElement.innerHTML) {
    const root = ReactDOM.createRoot(rootElement);

    root.render(
      <ThemeProvider defaultTheme="light">
        <QueryClientProvider client={queryClient}>
          <ClerkProvider
            publishableKey={import.meta.env.VITE_CLERK_PUBLISHABLE_KEY}
          >
            <RouterProvider router={router} />
          </ClerkProvider>
        </QueryClientProvider>
      </ThemeProvider>,
    );
  }
});

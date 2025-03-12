import "@hypr/ui/globals.css";
import "@stackflow/plugin-basic-ui/index.css";
import "./styles/globals.css";

import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { StrictMode, Suspense } from "react";
import ReactDOM from "react-dom/client";
import { HyprProvider } from "./contexts/hypr";
import { Stack } from "./stackflow";

const queryClient = new QueryClient();

const rootElement = document.getElementById("root")!;
if (!rootElement.innerHTML) {
  const root = ReactDOM.createRoot(rootElement);
  root.render(
    <StrictMode>
      <QueryClientProvider client={queryClient}>
        <Suspense>
          <HyprProvider>
            <Stack />
          </HyprProvider>
        </Suspense>
      </QueryClientProvider>
    </StrictMode>,
  );
}

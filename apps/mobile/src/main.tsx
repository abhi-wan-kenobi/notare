import { StrictMode } from "react";
import ReactDOM from "react-dom/client";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";

import { Stack } from "./stackflow";

import "@stackflow/plugin-basic-ui/index.css";
import "@hypr/ui/globals.css";
import "./styles/globals.css";

const queryClient = new QueryClient();

const rootElement = document.getElementById("root")!;
if (!rootElement.innerHTML) {
  const root = ReactDOM.createRoot(rootElement);
  root.render(
    <StrictMode>
      <QueryClientProvider client={queryClient}>
        <div>
          <Stack />
        </div>
      </QueryClientProvider>
    </StrictMode>,
  );
}

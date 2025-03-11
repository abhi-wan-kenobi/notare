import "@hypr/ui/globals.css";
import "@stackflow/plugin-basic-ui/index.css";
import "./styles/globals.css";

import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { StrictMode } from "react";
import ReactDOM from "react-dom/client";
import { Stack } from "./stackflow";

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

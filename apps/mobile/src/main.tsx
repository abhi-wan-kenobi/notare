import { StrictMode } from "react";
import ReactDOM from "react-dom/client";

import { Stack } from "./stackflow";

import "@stackflow/plugin-basic-ui/index.css";
import "@hypr/ui/globals.css";
import "./styles/globals.css";

const rootElement = document.getElementById("root")!;
if (!rootElement.innerHTML) {
  const root = ReactDOM.createRoot(rootElement);
  root.render(
    <StrictMode>
      <div>
        <div>123</div>
        <Stack />
      </div>
    </StrictMode>,
  );
}

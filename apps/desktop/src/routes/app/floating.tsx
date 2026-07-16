import { createFileRoute } from "@tanstack/react-router";

import { FloatingBarWindow } from "~/meeting-float/window";

export const Route = createFileRoute("/app/floating")({
  component: FloatingBarWindow,
});

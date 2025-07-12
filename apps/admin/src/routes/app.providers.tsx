import { createFileRoute } from "@tanstack/react-router";

export const Route = createFileRoute("/app/providers")({
  component: RouteComponent,
});

function RouteComponent() {
  return <div>Hello "/providers"!</div>;
}

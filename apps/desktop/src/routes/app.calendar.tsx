import { createFileRoute } from "@tanstack/react-router";

export const Route = createFileRoute("/app/calendar")({
  component: RouteComponent,
});

function RouteComponent() {
  return <div>Hello "/app/calendar"!</div>;
}

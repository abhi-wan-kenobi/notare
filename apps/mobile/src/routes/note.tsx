import { createFileRoute } from "@tanstack/react-router";

export const Route = createFileRoute("/note")({
  component: RouteComponent,
});

function RouteComponent() {
  return <div>Hello "/note"!</div>;
}

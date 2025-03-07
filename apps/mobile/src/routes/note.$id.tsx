import { createFileRoute } from "@tanstack/react-router";

export const Route = createFileRoute("/note/$id")({
  component: Component,
});

function Component() {
  return <div>Hello World</div>;
}

import { createFileRoute, useLoaderData } from "@tanstack/react-router";

export const Route = createFileRoute("/app/human/$id/sub")({
  component: Component,
});

function Component() {
  const { human } = useLoaderData({ from: "/app/human/$id" });
  return <pre>{JSON.stringify(human, null, 2)}</pre>;
}

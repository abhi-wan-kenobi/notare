import { createFileRoute, useLoaderData } from "@tanstack/react-router";

export const Route = createFileRoute("/app/organization/$id/main")({
  component: Component,
});

function Component() {
  const { organization } = useLoaderData({ from: "/app/organization/$id" });
  return <pre>{JSON.stringify(organization, null, 2)}</pre>;
}

import { createFileRoute } from "@tanstack/react-router";

import { client, getApiWebSessionById } from "../client";

export const Route = createFileRoute("/s/$id")({
  loader: async ({ params }) => {
    const session = await getApiWebSessionById({
      client: client,
      path: {
        id: params.id,
      },
    });

    return {
      session,
    };
  },
  component: RouteComponent,
});

function RouteComponent() {
  const { session } = Route.useLoaderData();

  return (
    <div className="h-screen w-screen p-8 bg-neutral-200">
      <div dangerouslySetInnerHTML={{ __html: JSON.stringify(session) }} />
    </div>
  );
}

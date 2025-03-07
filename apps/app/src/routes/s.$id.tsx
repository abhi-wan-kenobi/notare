import { createFileRoute } from "@tanstack/react-router";

import {
  client,
  getApiWebSessionById,
  getApiWebSessionByIdOptions,
} from "../client";
import { useQuery } from "@tanstack/react-query";

export const Route = createFileRoute("/s/$id")({
  loader: async ({ params }) => {
    const session = await getApiWebSessionById({
      client: client,
      meta: {
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

  const query = useQuery({
    queryKey: ["session"],
    queryFn: () =>
      getApiWebSessionById({
        client: client,
      }),
  });

  return (
    <div className="h-screen w-screen p-8 bg-neutral-200">
      <div dangerouslySetInnerHTML={{ __html: JSON.stringify(session) }} />
    </div>
  );
}

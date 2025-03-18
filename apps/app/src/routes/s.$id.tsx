import { createFileRoute } from "@tanstack/react-router";
import { client, getApiWebSessionById, type Session } from "../client";
import { Content, Header } from "../components/session";

interface RouteLoaderData {
  session: Session;
}

export const Route = createFileRoute("/s/$id")({
  loader: async ({ params }): Promise<RouteLoaderData> => {
    const response = await getApiWebSessionById({
      client: client,
      path: {
        id: params.id,
      },
    });

    if (!response.data) {
      throw new Error("Session not found");
    }

    return {
      session: response.data,
    };
  },
  component: Component,
});

function Component() {
  const { session } = Route.useLoaderData();

  return (
    <div className="relative flex flex-col h-screen w-screen overflow-hidden bg-white text-neutral-700">
      <div className="flex-1 flex flex-col overflow-hidden">
        <Header session={session} />
        <Content session={session} />
      </div>
    </div>
  );
}

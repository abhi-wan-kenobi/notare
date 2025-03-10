import { createFileRoute } from "@tanstack/react-router";
import { client, getApiWebSessionById, type Session } from "../client";
import { Header, Content } from "../components/session";

// Define the loader data type
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
  component: RouteComponent,
});

function RouteComponent() {
  const { session } = Route.useLoaderData();

  return (
    <div className="relative flex flex-col h-screen w-screen overflow-hidden bg-white text-neutral-700">
      {/* Main content area */}
      <div className="flex-1 relative overflow-hidden">
        {/* Content */}
        <Content session={session} />
        
        {/* Header - positioned above content but in DOM after for proper z-index */}
        <div className="absolute top-0 left-0 right-0 z-10 bg-white border-b border-neutral-200">
          <Header session={session} />
        </div>
      </div>
    </div>
  );
}

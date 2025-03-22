import { createFileRoute, useSearch } from "@tanstack/react-router";
import { zodValidator } from "@tanstack/zod-adapter";
import { z } from "zod";

const schema = z.object({
  tab: z.enum(["general", "calendar", "extensions", "templates", "team", "billing"]).default("general"),
});

const PATH = "/app/settings";
export const Route = createFileRoute(PATH)({
  validateSearch: zodValidator(schema),
  component: Component,
});

function Component() {
  const { tab } = useSearch({ from: PATH });

  return (
    <div className="flex flex-col gap-4">
      <div className="flex flex-col gap-2">
        <h1 className="text-2xl font-bold">Settings</h1>
        <h2 className="text-lg font-medium">{tab}</h2>
      </div>
    </div>
  );
}

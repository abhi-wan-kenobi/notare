import { useMutation } from "@tanstack/react-query";
import { createFileRoute } from "@tanstack/react-router";

import { authClient } from "@/lib/auth/client";

export const Route = createFileRoute("/app/")({
  component: Component,
});

function Component() {
  const m = useMutation({
    mutationFn: async ({ name }: { name: string }) => {
      const { data: apiKey, error } = await authClient.apiKey.create({
        name,
        expiresIn: null,
        prefix: "hypr_",
        metadata: {},
      });

      if (error) {
        throw error;
      }

      return apiKey;
    },
  });

  return (
    <div>
      <h1>Hyprnote</h1>
    </div>
  );
}

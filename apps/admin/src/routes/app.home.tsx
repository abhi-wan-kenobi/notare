import { Alert } from "@mantine/core";
import { useMutation } from "@tanstack/react-query";
import { createFileRoute, Link } from "@tanstack/react-router";
import { redirect } from "@tanstack/react-router";

import { authClient } from "@/lib/auth/client";

export const Route = createFileRoute("/app/home")({
  component: Component,
  beforeLoad: ({ context }) => {
    if (!context.userSession) {
      throw redirect({ to: "/login" });
    }
  },
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
      <Alert title="Hyprnote client not connected" color="blue">
        Go to{" "}
        <Link
          className="underline"
          to="/app/settings"
        >
          Settings
        </Link>{" "}
        to connect.
      </Alert>
    </div>
  );
}

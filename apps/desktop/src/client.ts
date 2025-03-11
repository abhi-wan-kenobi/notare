import type { NangoIntegration } from "@/types";
import { createClient, createConfig } from "@hypr/client";
import { commands as authCommands } from "@hypr/plugin-auth";
import { fetch } from "@hypr/utils";

export * from "@hypr/client/gen/sdk";
export * from "@hypr/client/gen/tanstack";
export * from "@hypr/client/gen/types";

export const baseUrl = import.meta.env.DEV
  ? "http://localhost:1234"
  : "https://app.hyprnote.com";

export const client = createClient(
  createConfig({
    fetch,
    auth: async () => {
      const token = await authCommands.getFromVault("remote-server");
      return token ?? undefined;
    },
    baseUrl,
  }),
);

export function getIntegrationURL(type: NangoIntegration) {
  return new URL(`/integrations?provider=${type}`, baseUrl).toString();
}

// We can't use SSE with generated client: https://github.com/hey-api/openapi-ts/issues/772
export async function enhance(req: any) {}

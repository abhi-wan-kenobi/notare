import { z } from "zod";

const envServerSchema = z.object({
  BASE_URL: z.string().default("http://localhost:3000"),
  ADMIN_EMAIL: z.string().email().default("yujonglee@hyprnote.com"),
  ORG_SLUG: z.string().default("hyprnote"),
  TELEMETRY: z.coerce.boolean().default(true),
});

const envClientSchema = z.object({
  BASE_URL: z.string().default("http://localhost:3000"),
});

type EnvServerType = z.infer<typeof envServerSchema>;
type EnvClientType = z.infer<typeof envClientSchema>;

declare global {
  namespace NodeJS {
    interface ProcessEnv extends EnvServerType {}
  }
}

const envServerParsed = envServerSchema.safeParse({
  BASE_URL: process.env.VITE_BASE_URL,
  ADMIN_EMAIL: process.env.ADMIN_EMAIL,
  ORG_SLUG: process.env.ORG_SLUG,
  TELEMETRY: process.env.TELEMETRY,
});

if (!envServerParsed.success) {
  throw new Error(
    `There is an error with the server environment variables: ${JSON.stringify(envServerParsed.error.issues)}`,
  );
}

const envClientParsed = envClientSchema.safeParse({
  BASE_URL: import.meta.env.VITE_BASE_URL,
});

if (!envClientParsed.success) {
  throw new Error(
    `There is an error with the client environment variables: ${JSON.stringify(envClientParsed.error.issues)}`,
  );
}

export const ENV_KEYS_SERVER = Object.keys(envServerSchema.shape) as [keyof EnvServerType, ...(keyof EnvServerType)[]];
export const envServerData = envServerParsed.data;
export const envClientData = envClientParsed.data;

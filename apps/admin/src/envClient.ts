import z from "zod";

const envSchema = z.object({
  VITE_BASE_URL: z.string().default("http://localhost:3000"),
});

export const envClientSchema = envSchema.parse({
  VITE_BASE_URL: import.meta.env.VITE_BASE_URL,
});

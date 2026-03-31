import { createFileRoute, useNavigate } from "@tanstack/react-router";
import { ChevronLeft } from "lucide-react";
import { useCallback } from "react";

import { commands as windowsCommands } from "@hypr/plugin-windows";

const INSTRUCTIONS = {
  "sign-in": {
    title: "Sign in to your account",
    description: "Complete sign-in in your browser, then return to Char.",
  },
  "calendar-connect": {
    title: "Connect your calendar",
    description:
      "Authorize calendar access in your browser, then return to Char.",
  },
} satisfies Record<string, { title: string; description: string }>;

export type InstructionType = keyof typeof INSTRUCTIONS;

export const Route = createFileRoute("/app/instruction")({
  validateSearch: (search): { type: InstructionType } => ({
    type: ((search as { type?: string }).type ?? "sign-in") as InstructionType,
  }),
  component: InstructionRoute,
});

function InstructionRoute() {
  const { type } = Route.useSearch();
  const { title, description } = INSTRUCTIONS[type] ?? INSTRUCTIONS["sign-in"];
  const navigate = useNavigate();

  const handleBack = useCallback(async () => {
    await navigate({ to: "/app/main" });
    await new Promise((resolve) => window.setTimeout(resolve, 100));
    await windowsCommands.windowRestoreFrameAnimated({ type: "main" });
  }, [navigate]);

  return (
    <div className="flex h-full flex-col select-none">
      <div
        data-tauri-drag-region
        className="flex shrink-0 items-center px-3 pt-12"
      >
        <button
          type="button"
          onClick={() => void handleBack()}
          className="flex h-8 w-8 items-center justify-center rounded-lg text-neutral-400 transition-colors hover:bg-neutral-100 hover:text-neutral-600"
        >
          <ChevronLeft className="h-5 w-5" />
        </button>
      </div>

      <div
        data-tauri-drag-region
        className="flex flex-1 flex-col items-center justify-center gap-6 p-8"
      >
        <img
          src="/assets/char-logo-icon-black.svg"
          alt=""
          className="h-10 w-10"
        />

        <div className="flex flex-col items-center gap-2 text-center">
          <h2 className="font-serif text-lg font-semibold">{title}</h2>
          <p className="text-sm text-neutral-500">{description}</p>
        </div>

        <div className="flex items-center gap-2">
          <div className="h-1.5 w-1.5 animate-bounce rounded-full bg-neutral-400 [animation-delay:-0.3s]" />
          <div className="h-1.5 w-1.5 animate-bounce rounded-full bg-neutral-400 [animation-delay:-0.15s]" />
          <div className="h-1.5 w-1.5 animate-bounce rounded-full bg-neutral-400" />
        </div>
      </div>
    </div>
  );
}

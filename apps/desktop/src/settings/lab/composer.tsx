import { useMutation } from "@tanstack/react-query";
import { platform } from "@tauri-apps/plugin-os";
import { ExternalLinkIcon, SparklesIcon } from "lucide-react";

import { commands as windowsCommands } from "@hypr/plugin-windows";
import { Button } from "@hypr/ui/components/ui/button";
import { sonnerToast } from "@hypr/ui/components/ui/toast";

export function ComposerSettings() {
  const isMacOS = platform() === "macos";

  const mutation = useMutation({
    mutationFn: async () => {
      const result = await windowsCommands.windowShow({ type: "composer" });

      if (result.status === "error") {
        throw new Error(String(result.error));
      }
    },
    onSuccess: () => {
      sonnerToast.success("Composer opened");
    },
    onError: (error) => {
      sonnerToast.error(error.message);
    },
  });

  return (
    <div className="flex items-center justify-between gap-4">
      <div className="flex-1">
        <h3 className="mb-1 text-sm font-medium">Composer</h3>
        <p className="text-xs text-neutral-600">
          Open the spotlight-style chat launcher in its own panel window.
        </p>
      </div>

      <Button
        size="sm"
        type="button"
        variant="outline"
        onClick={() => mutation.mutate()}
        disabled={!isMacOS || mutation.isPending}
      >
        {isMacOS ? (
          <>
            <SparklesIcon className="mr-2 size-4" />
            {mutation.isPending ? "Opening..." : "Open"}
          </>
        ) : (
          <>
            <ExternalLinkIcon className="mr-2 size-4" />
            macOS Only
          </>
        )}
      </Button>
    </div>
  );
}

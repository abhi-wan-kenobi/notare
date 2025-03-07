import type { ActivityLoaderArgs } from "@stackflow/config";

export function homeActivityLoader({
  params,
}: ActivityLoaderArgs<"HomeActivity">) {
  const { todo } = params;

  return {
    todo,
  };
}

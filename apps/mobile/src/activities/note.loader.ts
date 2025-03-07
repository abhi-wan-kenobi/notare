import type { ActivityLoaderArgs } from "@stackflow/config";

export function noteActivityLoader({
  params,
}: ActivityLoaderArgs<"NoteActivity">) {
  const { todo } = params;

  return {
    todo,
  };
}

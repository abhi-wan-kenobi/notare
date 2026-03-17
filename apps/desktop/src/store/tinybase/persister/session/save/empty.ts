import {
  buildSessionPath,
  type TablesContent,
  type WriteOperation,
} from "~/store/tinybase/persister/shared";

export function buildEmptySessionDeleteOps(
  tables: TablesContent,
  dataDir: string,
  emptySessionIds: Set<string>,
): WriteOperation[] {
  if (emptySessionIds.size === 0) {
    return [];
  }

  const paths: string[] = [];

  for (const id of emptySessionIds) {
    const session = tables.sessions?.[id];
    paths.push(
      buildSessionPath(dataDir, id, (session?.folder_id as string) ?? ""),
    );
  }

  return [{ type: "delete", paths }];
}

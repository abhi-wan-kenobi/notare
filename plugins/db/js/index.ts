import { Channel, invoke } from "@tauri-apps/api/core";

export type QueryEvent<T = Record<string, unknown>> =
  | { event: "result"; data: T[] }
  | { event: "error"; data: string };

export async function execute<T = Record<string, unknown>>(
  sql: string,
  params: unknown[] = [],
): Promise<T[]> {
  return invoke("plugin:db|execute", { sql, params });
}

export async function subscribe<T = Record<string, unknown>>(
  sql: string,
  params: unknown[],
  options: {
    onData: (rows: T[]) => void;
    onError?: (error: string) => void;
  },
): Promise<() => void> {
  const channel = new Channel<QueryEvent<T>>();

  channel.onmessage = (event) => {
    if (event.event === "result") {
      options.onData(event.data);
      return;
    }

    options.onError?.(event.data);
  };

  const subscriptionId: string = await invoke("plugin:db|subscribe", {
    sql,
    params,
    onEvent: channel,
  });

  return () => {
    invoke("plugin:db|unsubscribe", { subscriptionId }).catch(() => {});
  };
}

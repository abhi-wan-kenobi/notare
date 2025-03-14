import { createContext, useContext, useRef } from "react";
import { useStore } from "zustand";
import { useShallow } from "zustand/shallow";

import { createSessionsStore, SessionsStore } from "@/stores";

const SessionsContext = createContext<
  ReturnType<
    typeof createSessionsStore
  > | null
>(null);

export const SessionsProvider = ({
  children,
  store,
}: {
  children: React.ReactNode;
  store?: SessionsStore;
}) => {
  const storeRef = useRef<ReturnType<typeof createSessionsStore> | null>(
    null,
  );
  if (!storeRef.current) {
    storeRef.current = store || createSessionsStore();
  }

  return (
    <SessionsContext.Provider value={storeRef.current}>
      {children}
    </SessionsContext.Provider>
  );
};

export const useSessions = <T,>(
  selector: Parameters<
    typeof useStore<ReturnType<typeof createSessionsStore>, T>
  >[1],
) => {
  const store = useContext(SessionsContext);

  if (!store) {
    throw new Error(
      "'useSessions' must be used within a 'SessionsProvider'",
    );
  }

  return useStore(store, useShallow(selector));
};

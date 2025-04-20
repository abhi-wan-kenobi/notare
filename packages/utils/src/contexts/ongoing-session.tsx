import React, { createContext, useContext, useEffect, useRef } from "react";
import { useStore } from "zustand";
import { useShallow } from "zustand/shallow";

import { events as listenerEvents } from "@hypr/plugin-listener";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { createOngoingSessionStore, createSessionsStore } from "../stores";

const OngoingSessionContext = createContext<
  ReturnType<
    typeof createOngoingSessionStore
  > | null
>(null);

export const OngoingSessionProvider = ({
  children,
  sessionsStore,
}: {
  children: React.ReactNode;
  sessionsStore: ReturnType<typeof createSessionsStore>;
}) => {
  const storeRef = useRef<ReturnType<typeof createOngoingSessionStore> | null>(
    null,
  );
  if (!storeRef.current) {
    storeRef.current = createOngoingSessionStore(sessionsStore);
  }

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    const currentWindow = getCurrentWebviewWindow();

    listenerEvents.statusEvent(currentWindow).listen(({ payload }) => {
      let api = storeRef.current?.getState();
      api?.setStatus(payload);
    }).then((fn) => {
      unlisten = fn;
    });

    return () => unlisten?.();
  }, []);

  return (
    <OngoingSessionContext.Provider value={storeRef.current}>
      {children}
    </OngoingSessionContext.Provider>
  );
};

export const useOngoingSession = <T,>(
  selector: Parameters<
    typeof useStore<ReturnType<typeof createOngoingSessionStore>, T>
  >[1],
) => {
  const store = useContext(OngoingSessionContext);

  if (!store) {
    throw new Error(
      "'useOngoingSession' must be used within a 'OngoingSessionProvider'",
    );
  }

  return useStore(store, useShallow(selector));
};

import { createContext, useContext } from "react";

import { useInspector } from "./inspector";
import { useLeftSidebar } from "./leftsidebar";
import { useSettings } from "./settings";

import { useChatMode } from "~/chat/state/use-chat-mode";

interface ShellContextType {
  chat: ReturnType<typeof useChatMode>;
  inspector: ReturnType<typeof useInspector>;
  leftsidebar: ReturnType<typeof useLeftSidebar>;
  settings: ReturnType<typeof useSettings>;
}

const ShellContext = createContext<ShellContextType | null>(null);

export function ShellProvider({ children }: { children: React.ReactNode }) {
  const chat = useChatMode();
  const inspector = useInspector(chat);
  const leftsidebar = useLeftSidebar();
  const settings = useSettings();

  return (
    <ShellContext.Provider value={{ chat, inspector, leftsidebar, settings }}>
      {children}
    </ShellContext.Provider>
  );
}

export function useShell() {
  const context = useContext(ShellContext);
  if (!context) {
    throw new Error("'useShell' must be used within 'ShellProvider'");
  }
  return context;
}

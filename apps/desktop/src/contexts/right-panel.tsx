import { createContext, useContext, useState, type ReactNode } from "react";

interface RightPanelContextType {
  isExpanded: boolean;
  togglePanel: () => void;
}

const RightPanelContext = createContext<RightPanelContextType | null>(null);

export function RightPanelProvider({ children }: { children: ReactNode }) {
  const [isExpanded, setIsExpanded] = useState(false);

  const togglePanel = () => {
    setIsExpanded((prev) => !prev);
  };

  return (
    <RightPanelContext.Provider value={{ isExpanded, togglePanel }}>
      {children}
    </RightPanelContext.Provider>
  );
}

export function useRightPanel() {
  const context = useContext(RightPanelContext);
  if (!context) {
    throw new Error("useRightPanel must be used within a RightPanelProvider");
  }
  return context;
}

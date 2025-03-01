import { createContext, useContext, useState, useCallback } from "react";
import { useHotkeys } from "react-hotkeys-hook";

interface RightPanelContextType {
  isExpanded: boolean;
  togglePanel: () => void;
  hidePanel: () => void;
}

const RightPanelContext = createContext<RightPanelContextType | null>(null);

export function RightPanelProvider({
  children,
}: {
  children: React.ReactNode;
}) {
  const [isExpanded, setIsExpanded] = useState(false);

  const hidePanel = useCallback(() => {
    setIsExpanded(false);
  }, []);

  const togglePanel = useCallback(() => {
    setIsExpanded((prev) => !prev);
  }, []);

  useHotkeys(
    "mod+r",
    (event) => {
      event.preventDefault();
      togglePanel();
    },
    {
      enableOnFormTags: true,
      enableOnContentEditable: true,
    },
  );

  return (
    <RightPanelContext.Provider value={{ isExpanded, togglePanel, hidePanel }}>
      {children}
    </RightPanelContext.Provider>
  );
}

export function useRightPanel() {
  const context = useContext(RightPanelContext);
  if (!context) {
    throw new Error("useRightPanel must be used within RightPanelProvider");
  }
  return context;
}

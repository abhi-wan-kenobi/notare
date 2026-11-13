import { useEffect, useState } from "react";

const MAIN_CONTENT_PANEL_SELECTOR = "[data-main-content-panel]";

/**
 * Horizontal distance from the window's centre to the centre of the main
 * content panel, so overlays (command palette, status banner) centre over the
 * content rather than over the window. Measures the content panel by name:
 * the sidebar is the first `[data-panel-id]` in DOM order, not the content.
 */
export function useMainContentCenterOffset() {
  const [contentOffset, setContentOffset] = useState(0);

  useEffect(() => {
    const computeOffset = () => {
      const content = document.querySelector(MAIN_CONTENT_PANEL_SELECTOR);
      if (!content) {
        setContentOffset(0);
        return;
      }

      const rect = content.getBoundingClientRect();
      setContentOffset(rect.left + rect.width / 2 - window.innerWidth / 2);
    };

    computeOffset();
    window.addEventListener("resize", computeOffset);

    const resizeObserver = new ResizeObserver(computeOffset);
    const content = document.querySelector(MAIN_CONTENT_PANEL_SELECTOR);
    if (content) {
      resizeObserver.observe(content);
    }

    return () => {
      window.removeEventListener("resize", computeOffset);
      resizeObserver.disconnect();
    };
  }, []);

  return contentOffset;
}

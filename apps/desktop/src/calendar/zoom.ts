import { useCallback, useState } from "react";

/**
 * Calendar zoom, from zoomed-out to zoomed-in. "month" is the 7-column month
 * grid; the others are horizontally scrolling day columns. The window width
 * still caps how many columns fit, so a narrow window may show fewer days
 * than the chosen level.
 */
export const ZOOM_LEVELS = ["month", "week", "3day", "day"] as const;
export type ZoomLevel = (typeof ZOOM_LEVELS)[number];

const LEVEL_COLS: Record<ZoomLevel, number> = {
  month: 7,
  week: 7,
  "3day": 3,
  day: 1,
};

const STORAGE_KEY = "notare.calendar.zoom";

export type CalendarLayout = { isMonthView: boolean; cols: number };

/**
 * `widthCols` is what the container can fit (7 / 4 / 2 / 1). The month grid
 * needs all 7; below that the month level falls back to as many day columns
 * as fit, exactly like the pre-zoom auto behavior.
 */
export function resolveCalendarLayout(
  zoom: ZoomLevel,
  widthCols: number,
): CalendarLayout {
  if (zoom === "month" && widthCols >= 7) {
    return { isMonthView: true, cols: 7 };
  }
  return { isMonthView: false, cols: Math.min(LEVEL_COLS[zoom], widthCols) };
}

/** `direction` +1 zooms in (fewer days), -1 zooms out. Clamps at the ends. */
export function stepZoom(zoom: ZoomLevel, direction: 1 | -1): ZoomLevel {
  const index = ZOOM_LEVELS.indexOf(zoom) + direction;
  return ZOOM_LEVELS[Math.max(0, Math.min(ZOOM_LEVELS.length - 1, index))];
}

function readStoredZoom(): ZoomLevel {
  const stored = localStorage.getItem(STORAGE_KEY);
  return ZOOM_LEVELS.find((level) => level === stored) ?? "month";
}

export function useCalendarZoom() {
  const [zoom, setZoom] = useState<ZoomLevel>(readStoredZoom);

  const zoomBy = useCallback((direction: 1 | -1) => {
    setZoom((current) => {
      const next = stepZoom(current, direction);
      localStorage.setItem(STORAGE_KEY, next);
      return next;
    });
  }, []);

  return { zoom, zoomBy };
}

/**
 * Trackpad pinch arrives as `wheel` events with `ctrlKey` set (macOS and
 * Windows WebView2); Ctrl/Cmd + mouse wheel does the same. Small deltas are
 * accumulated so one pinch gesture moves one level, not four.
 */
export const PINCH_STEP_THRESHOLD = 40;

export function createPinchAccumulator(onStep: (direction: 1 | -1) => void) {
  let accumulated = 0;
  return (event: Pick<WheelEvent, "ctrlKey" | "metaKey" | "deltaY">) => {
    if (!event.ctrlKey && !event.metaKey) {
      // A plain scroll ends any gesture in progress; a leftover partial sum
      // would make the next pinch step early.
      accumulated = 0;
      return false;
    }
    accumulated += event.deltaY;
    if (Math.abs(accumulated) >= PINCH_STEP_THRESHOLD) {
      // Pinch-out / wheel-up has negative deltaY → zoom in.
      onStep(accumulated < 0 ? 1 : -1);
      accumulated = 0;
    }
    return true;
  };
}

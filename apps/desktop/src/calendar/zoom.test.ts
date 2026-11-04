import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import {
  createPinchAccumulator,
  PINCH_STEP_THRESHOLD,
  resolveCalendarLayout,
  stepZoom,
  useCalendarZoom,
} from "./zoom";

describe("resolveCalendarLayout", () => {
  it("keeps the pre-zoom behavior at the default level", () => {
    expect(resolveCalendarLayout("month", 7)).toEqual({
      isMonthView: true,
      cols: 7,
    });
    // Too narrow for a month grid → as many day columns as fit.
    expect(resolveCalendarLayout("month", 4)).toEqual({
      isMonthView: false,
      cols: 4,
    });
  });

  it("zoomed-in levels show fewer day columns, capped by the width", () => {
    expect(resolveCalendarLayout("week", 7)).toEqual({
      isMonthView: false,
      cols: 7,
    });
    expect(resolveCalendarLayout("3day", 7).cols).toBe(3);
    expect(resolveCalendarLayout("day", 7).cols).toBe(1);
    expect(resolveCalendarLayout("3day", 2).cols).toBe(2);
  });
});

describe("stepZoom", () => {
  it("moves one level and clamps at both ends", () => {
    expect(stepZoom("month", 1)).toBe("week");
    expect(stepZoom("week", -1)).toBe("month");
    expect(stepZoom("month", -1)).toBe("month");
    expect(stepZoom("day", 1)).toBe("day");
  });
});

describe("createPinchAccumulator", () => {
  it("ignores plain scrolling so horizontal day scrolling keeps working", () => {
    const onStep = vi.fn();
    const handle = createPinchAccumulator(onStep);
    expect(handle({ ctrlKey: false, metaKey: false, deltaY: -500 })).toBe(
      false,
    );
    expect(onStep).not.toHaveBeenCalled();
  });

  it("turns one pinch gesture of many small deltas into a single step", () => {
    const onStep = vi.fn();
    const handle = createPinchAccumulator(onStep);
    // A macOS trackpad pinch-out: ~2px deltas, ctrlKey set by the OS.
    for (let i = 0; i < PINCH_STEP_THRESHOLD / 2; i++) {
      handle({ ctrlKey: true, metaKey: false, deltaY: -2 });
    }
    expect(onStep).toHaveBeenCalledTimes(1);
    expect(onStep).toHaveBeenCalledWith(1);
  });

  it("Cmd/Ctrl + wheel down zooms out", () => {
    const onStep = vi.fn();
    const handle = createPinchAccumulator(onStep);
    handle({ ctrlKey: false, metaKey: true, deltaY: 100 });
    expect(onStep).toHaveBeenCalledWith(-1);
  });
});

describe("useCalendarZoom", () => {
  beforeEach(() => localStorage.clear());

  it("remembers the chosen level across mounts", () => {
    const first = renderHook(() => useCalendarZoom());
    act(() => first.result.current.zoomBy(1));
    act(() => first.result.current.zoomBy(1));
    expect(first.result.current.zoom).toBe("3day");
    first.unmount();

    const second = renderHook(() => useCalendarZoom());
    expect(second.result.current.zoom).toBe("3day");
  });

  it("falls back to month on an unknown stored value", () => {
    localStorage.setItem("notare.calendar.zoom", "fortnight");
    const { result } = renderHook(() => useCalendarZoom());
    expect(result.current.zoom).toBe("month");
  });
});

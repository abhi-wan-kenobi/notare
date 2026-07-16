import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { RootErrorBoundary } from "./root-error-boundary";

function Bomb(): never {
  throw new Error("kaboom during first render");
}

describe("RootErrorBoundary", () => {
  beforeEach(() => {
    // React logs caught render errors; keep test output clean.
    vi.spyOn(console, "error").mockImplementation(() => {});
  });

  afterEach(() => {
    // vitest globals are off, so testing-library's automatic cleanup does not
    // run; without this, earlier renders leak into later queries.
    cleanup();
    vi.restoreAllMocks();
  });

  it("renders children when nothing throws", () => {
    render(
      <RootErrorBoundary>
        <div>healthy app</div>
      </RootErrorBoundary>,
    );

    expect(screen.getByText("healthy app")).toBeTruthy();
    expect(screen.queryByTestId("root-error-boundary")).toBeNull();
  });

  it("shows the recovery UI instead of a blank screen when a child throws during first render", () => {
    render(
      <RootErrorBoundary>
        <Bomb />
      </RootErrorBoundary>,
    );

    expect(screen.getByTestId("root-error-boundary")).toBeTruthy();
    expect(screen.getByText("Notare")).toBeTruthy();
    expect(screen.getByText("Something went wrong")).toBeTruthy();
    expect(
      screen.getByText("kaboom during first render"),
    ).toBeTruthy();
  });

  it("reloads the window when the Reload button is clicked", () => {
    const reload = vi.fn();
    const originalLocation = window.location;
    Object.defineProperty(window, "location", {
      configurable: true,
      value: { ...originalLocation, reload },
    });

    try {
      render(
        <RootErrorBoundary>
          <Bomb />
        </RootErrorBoundary>,
      );

      fireEvent.click(screen.getByRole("button", { name: "Reload Notare" }));
      expect(reload).toHaveBeenCalledTimes(1);
    } finally {
      Object.defineProperty(window, "location", {
        configurable: true,
        value: originalLocation,
      });
    }
  });

  it("stringifies non-Error throwables", () => {
    function StringBomb(): never {
      // eslint-disable-next-line @typescript-eslint/only-throw-error
      throw "plain string failure";
    }

    render(
      <RootErrorBoundary>
        <StringBomb />
      </RootErrorBoundary>,
    );

    expect(screen.getByText("plain string failure")).toBeTruthy();
  });
});

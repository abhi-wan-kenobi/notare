import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { ContentErrorBoundary } from "./content-error-boundary";

function Bomb(): never {
  throw new Error("kaboom in a single surface");
}

describe("ContentErrorBoundary", () => {
  beforeEach(() => {
    // React logs caught render errors; keep test output clean.
    vi.spyOn(console, "error").mockImplementation(() => {});
  });

  afterEach(() => {
    cleanup();
    vi.restoreAllMocks();
  });

  it("renders children when nothing throws", () => {
    render(
      <ContentErrorBoundary>
        <div>healthy surface</div>
      </ContentErrorBoundary>,
    );

    expect(screen.getByText("healthy surface")).toBeTruthy();
    expect(screen.queryByTestId("content-error-boundary")).toBeNull();
  });

  it("shows a local recovery card instead of unmounting the rest of the window", () => {
    render(
      <div>
        <div data-testid="sibling-chrome">sidebar / tab bar</div>
        <ContentErrorBoundary>
          <Bomb />
        </ContentErrorBoundary>
      </div>,
    );

    // The crash is contained: the card shows...
    expect(screen.getByTestId("content-error-boundary")).toBeTruthy();
    expect(screen.getByText("This section hit a problem")).toBeTruthy();
    expect(screen.getByText("kaboom in a single surface")).toBeTruthy();
    // ...and everything outside the boundary is untouched.
    expect(screen.getByTestId("sibling-chrome")).toBeTruthy();
  });

  it("recovers on 'Try again' without a full reload", () => {
    let shouldThrow = true;
    function Flaky() {
      if (shouldThrow) {
        throw new Error("flaky render");
      }
      return <div>recovered</div>;
    }

    const { rerender } = render(
      <ContentErrorBoundary>
        <Flaky />
      </ContentErrorBoundary>,
    );

    expect(screen.getByTestId("content-error-boundary")).toBeTruthy();

    // Fix the underlying condition (as a real retry would: the transient
    // cause is gone), then retry.
    shouldThrow = false;
    fireEvent.click(screen.getByRole("button", { name: "Try again" }));
    rerender(
      <ContentErrorBoundary>
        <Flaky />
      </ContentErrorBoundary>,
    );

    expect(screen.queryByTestId("content-error-boundary")).toBeNull();
    expect(screen.getByText("recovered")).toBeTruthy();
  });

  it("clears the caught error automatically when resetKey changes", () => {
    const { rerender } = render(
      <ContentErrorBoundary resetKey="settings">
        <Bomb />
      </ContentErrorBoundary>,
    );

    expect(screen.getByTestId("content-error-boundary")).toBeTruthy();

    // Simulates navigating away from the crashed tab and back to a
    // *different* one - the caller remounts with a new resetKey.
    rerender(
      <ContentErrorBoundary resetKey="sessions">
        <div>a different, healthy tab</div>
      </ContentErrorBoundary>,
    );

    expect(screen.queryByTestId("content-error-boundary")).toBeNull();
    expect(screen.getByText("a different, healthy tab")).toBeTruthy();
  });

  it("stringifies non-Error throwables", () => {
    function StringBomb(): never {
      // eslint-disable-next-line @typescript-eslint/only-throw-error
      throw "plain string failure";
    }

    render(
      <ContentErrorBoundary>
        <StringBomb />
      </ContentErrorBoundary>,
    );

    expect(screen.getByText("plain string failure")).toBeTruthy();
  });
});

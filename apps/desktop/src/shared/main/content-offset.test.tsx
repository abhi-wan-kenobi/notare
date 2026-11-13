import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { useMainContentCenterOffset } from "./content-offset";

const rect = (left: number, width: number) =>
  ({ left, width, right: left + width }) as DOMRect;

function Probe() {
  return <output>{useMainContentCenterOffset()}</output>;
}

describe("useMainContentCenterOffset", () => {
  beforeEach(() => {
    vi.stubGlobal(
      "ResizeObserver",
      class {
        observe() {}
        disconnect() {}
      },
    );
    vi.stubGlobal("innerWidth", 1440);
  });
  afterEach(() => {
    cleanup();
    vi.unstubAllGlobals();
  });

  // 0.7.0: the hook measured the first [data-panel-id], which is the sidebar,
  // so the command palette opened centred on the sidebar and half off-screen.
  it("centres on the content panel, not the sidebar that precedes it", () => {
    const sidebar = document.createElement("div");
    sidebar.setAttribute("data-panel-id", "classic-main-sidebar-left");
    sidebar.getBoundingClientRect = () => rect(0, 208);
    const content = document.createElement("div");
    content.setAttribute("data-panel-id", "classic-main-content");
    const inner = document.createElement("div");
    inner.setAttribute("data-main-content-panel", "");
    inner.getBoundingClientRect = () => rect(208, 1232);
    content.append(inner);
    document.body.append(sidebar, content);

    render(<Probe />);

    // content centre 824 − window centre 720
    expect(screen.getByRole("status").textContent).toBe("104");
    sidebar.remove();
    content.remove();
  });
});

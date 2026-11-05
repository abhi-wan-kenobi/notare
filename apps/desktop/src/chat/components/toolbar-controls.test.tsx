import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import type { ReactNode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@hypr/ui/components/ui/button", () => ({
  Button: ({
    children,
    className,
    ...props
  }: React.ButtonHTMLAttributes<HTMLButtonElement>) => (
    <button className={className} type="button" {...props}>
      {children}
    </button>
  ),
}));

vi.mock("@hypr/ui/components/ui/dropdown-menu", () => ({
  AppFloatingPanel: ({
    children,
    className,
  }: {
    children: ReactNode;
    className?: string;
  }) => (
    <div className={className} data-testid="chat-history-panel">
      {children}
    </div>
  ),
  DropdownMenu: ({ children }: { children: ReactNode }) => (
    <div>{children}</div>
  ),
  DropdownMenuContent: ({
    avoidCollisions,
    children,
    className,
    side,
    sideOffset,
  }: {
    avoidCollisions?: boolean;
    children: ReactNode;
    className?: string;
    side?: string;
    sideOffset?: number;
  }) => (
    <div
      className={className}
      data-avoid-collisions={String(avoidCollisions)}
      data-side={side}
      data-side-offset={sideOffset}
      data-testid="chat-history-menu"
    >
      {children}
    </div>
  ),
  DropdownMenuTrigger: ({ children }: { children: ReactNode }) => (
    <>{children}</>
  ),
}));

vi.mock("~/chat/store/queries", () => ({
  useRecentChatGroups: () => [],
}));

import { ChatToolbarControls } from "./toolbar-controls";

describe("ChatToolbarControls", () => {
  beforeEach(() => {
    cleanup();
  });

  it("renders the dark chat history trigger as a pill button", () => {
    render(
      <ChatToolbarControls
        currentChatGroupId={undefined}
        onNewChat={vi.fn()}
        onSelectChat={vi.fn()}
        surface="dark"
      />,
    );

    const historyButton = screen.getByRole("button", { name: "Chat history" });
    expect(historyButton.className).toContain("rounded-full");
    expect(historyButton.className).toContain("h-8");
    expect(historyButton.className).toContain("w-auto");
    expect(historyButton.className).toContain("gap-1.5");
    expect(historyButton.className).toContain("hover:bg-primary-foreground/14");
    expect(screen.queryByText("Ask Notare AI anything")).toBeNull();
  });

  it("renders the light chat history trigger without title text", () => {
    const { container } = render(
      <ChatToolbarControls
        currentChatGroupId={undefined}
        onNewChat={vi.fn()}
        onSelectChat={vi.fn()}
        surface="light"
      />,
    );

    const historyButton = screen.getByRole("button", { name: "Chat history" });
    expect(container.firstElementChild?.className).toContain("px-3");
    expect(container.firstElementChild?.className).not.toContain("pl-2");
    expect(container.firstElementChild?.className).not.toContain("pr-2");
    expect(historyButton.className).toContain("-ml-2");
    expect(historyButton.className).toContain("h-8");
    expect(historyButton.className).toContain("w-auto");
    expect(historyButton.className).toContain("gap-1.5");
    expect(historyButton.className).toContain("text-muted-foreground");
    expect(historyButton.className).toContain("hover:bg-muted/80");
    expect(historyButton.textContent).toBe("");
    expect(screen.queryByText("Ask Notare AI anything")).toBeNull();
  });

  it("keeps the history menu below the trigger and scrolls inside the panel", () => {
    render(
      <ChatToolbarControls
        currentChatGroupId={undefined}
        onNewChat={vi.fn()}
        onSelectChat={vi.fn()}
        surface="light"
      />,
    );

    const menu = screen.getByTestId("chat-history-menu");
    const panel = screen.getByTestId("chat-history-panel");

    expect(menu.dataset.side).toBe("bottom");
    expect(menu.dataset.sideOffset).toBe("4");
    expect(menu.dataset.avoidCollisions).toBe("false");
    expect(menu.className).toContain("w-72");
    expect(menu.className).toContain("max-w-[calc(100vw-2rem)]");
    expect(panel.className).toContain("max-h-80");
    expect(panel.className).toContain("overflow-y-auto");
  });

  it("renders dark toolbar action buttons as circles without tooltips", () => {
    render(
      <ChatToolbarControls
        currentChatGroupId={undefined}
        onClose={vi.fn()}
        onNewChat={vi.fn()}
        onSelectChat={vi.fn()}
        surface="dark"
      />,
    );

    const newChatButton = screen.getByRole("button", { name: "New chat" });
    const closeButton = screen.getByRole("button", { name: "Close chat" });

    expect(newChatButton.className).toContain("rounded-full");
    expect(newChatButton.className).toContain(
      "hover:!bg-primary-foreground/14",
    );
    expect(newChatButton.getAttribute("title")).toBeNull();
    expect(closeButton.className).toContain("rounded-full");
    expect(closeButton.className).toContain("hover:!bg-primary-foreground/14");
    expect(closeButton.getAttribute("title")).toBeNull();
  });

  it("uses the shared toolbar padding and closes the inspector", () => {
    const onClose = vi.fn();
    const { container } = render(
      <ChatToolbarControls
        currentChatGroupId={undefined}
        onClose={onClose}
        onNewChat={vi.fn()}
        onSelectChat={vi.fn()}
        surface="light"
      />,
    );

    const historyButton = screen.getByRole("button", { name: "Chat history" });

    expect(container.firstElementChild?.className).toContain("px-3");
    expect(container.firstElementChild?.className).not.toContain("px-5");
    expect(container.firstElementChild?.className).not.toContain("px-2");
    expect(container.firstElementChild?.className).not.toContain("pr-0");
    expect(container.firstElementChild?.className).not.toContain("pr-1");
    const actions = container.querySelector("[data-chat-toolbar-actions]");
    expect(actions?.className).toContain("gap-0");
    expect(actions?.className).not.toContain("gap-1");
    expect(historyButton.className).toContain("-ml-2");
    expect(historyButton.className).toContain("h-8");
    expect(historyButton.className).toContain("w-auto");
    expect(screen.queryByText("Ask Notare AI anything")).toBeNull();
    const closeButton = screen.getByRole("button", { name: "Close chat" });
    expect(closeButton.className).toContain("hover:!bg-muted/80");

    fireEvent.click(closeButton);

    expect(onClose).toHaveBeenCalled();
  });
});
